use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use parking_lot::Mutex;
use ta_protocol::wire::RunId;

#[cfg(test)]
#[path = "claims_tests.rs"]
mod claims_tests;

const DEFAULT_CLAIM_TTL: Duration = Duration::from_secs(30 * 60);

pub type CapsuleId = RunId;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ClaimRecord {
    pub capsule_id: CapsuleId,
    pub files: Vec<PathBuf>,
    pub claimed_at: SystemTime,
    pub expires_at: SystemTime,
    pub kind: ClaimKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ClaimKind {
    Write,
}

#[derive(Clone, Debug)]
pub struct ConflictWarning {
    pub requesting_capsule: CapsuleId,
    pub conflicts: Vec<ClaimConflict>,
}

#[derive(Clone, Debug)]
pub struct ClaimConflict {
    pub file: PathBuf,
    pub holding_capsule: CapsuleId,
    pub holding_since: Duration,
    pub holding_kind: ClaimKind,
}

#[derive(thiserror::Error, Debug)]
pub enum ClaimError {
    #[error("io error normalizing path: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid path: {0}")]
    InvalidPath(String),
}

#[derive(Clone)]
pub struct ClaimRegistry {
    inner: Arc<Mutex<ClaimRegistryInner>>,
    default_ttl: Duration,
}

#[must_use]
pub struct ClaimHandle {
    capsule_id: CapsuleId,
    registry: Arc<Mutex<ClaimRegistryInner>>,
    files: Vec<PathBuf>,
    token: u64,
}

impl ClaimHandle {
    pub fn capsule_id(&self) -> &CapsuleId {
        &self.capsule_id
    }

    pub fn files(&self) -> &[PathBuf] {
        &self.files
    }

    pub fn refresh(&self, additional: Duration) {
        let mut inner = self.registry.lock();
        inner.refresh_if_current(&self.capsule_id, self.token, additional);
    }

    pub fn release(&self) {
        let mut inner = self.registry.lock();
        inner.release_if_current(&self.capsule_id, self.token);
    }
}

impl Drop for ClaimHandle {
    fn drop(&mut self) {
        self.release();
    }
}

impl ClaimRegistry {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(ClaimRegistryInner::default())),
            default_ttl: DEFAULT_CLAIM_TTL,
        }
    }

    pub fn with_default_ttl(mut self, ttl: Duration) -> Self {
        self.default_ttl = ttl;
        self
    }

    pub fn claim(
        &self,
        capsule_id: CapsuleId,
        files: Vec<PathBuf>,
        ttl: Option<Duration>,
        kind: ClaimKind,
    ) -> Result<(ClaimHandle, Option<ConflictWarning>), ClaimError> {
        let files = normalize_file_list(&files, false)?;
        let now = SystemTime::now();
        let ttl = ttl.unwrap_or(self.default_ttl);
        let expires_at = checked_add(now, ttl)?;

        let mut inner = self.inner.lock();
        inner.sweep_expired_at(now);
        let warning = inner.conflict_warning(&capsule_id, &files, now);
        inner.remove_claim(&capsule_id);

        let token = inner.next_token();
        let record = ClaimRecord {
            capsule_id: capsule_id.clone(),
            files: files.clone(),
            claimed_at: now,
            expires_at,
            kind,
        };
        inner.insert_claim(record, token);

        Ok((
            ClaimHandle {
                capsule_id,
                registry: Arc::clone(&self.inner),
                files,
                token,
            },
            warning,
        ))
    }

    pub fn check_conflict(
        &self,
        capsule_id: &CapsuleId,
        files: &[PathBuf],
        _kind: ClaimKind,
    ) -> Result<Option<ConflictWarning>, ClaimError> {
        let files = normalize_file_list(files, true)?;
        if files.is_empty() {
            return Ok(None);
        }

        let now = SystemTime::now();
        let mut inner = self.inner.lock();
        inner.sweep_expired_at(now);
        Ok(inner.conflict_warning(capsule_id, &files, now))
    }

    pub fn active_claims(&self) -> Vec<ClaimRecord> {
        let now = SystemTime::now();
        let mut inner = self.inner.lock();
        inner.sweep_expired_at(now);
        inner
            .claims
            .values()
            .map(|claim| claim.record.clone())
            .collect()
    }

    pub fn sweep_expired(&self) -> usize {
        let mut inner = self.inner.lock();
        inner.sweep_expired_at(SystemTime::now())
    }
}

impl Default for ClaimRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Default)]
struct ClaimRegistryInner {
    claims: BTreeMap<CapsuleId, StoredClaim>,
    by_file: BTreeMap<PathBuf, Vec<CapsuleId>>,
    next_token: u64,
}

struct StoredClaim {
    record: ClaimRecord,
    token: u64,
}

impl ClaimRegistryInner {
    fn next_token(&mut self) -> u64 {
        self.next_token = self.next_token.saturating_add(1);
        self.next_token
    }

    fn insert_claim(&mut self, record: ClaimRecord, token: u64) {
        for file in &record.files {
            let holders = self.by_file.entry(file.clone()).or_default();
            holders.push(record.capsule_id.clone());
            holders.sort();
            holders.dedup();
        }
        self.claims
            .insert(record.capsule_id.clone(), StoredClaim { record, token });
    }

    fn remove_claim(&mut self, capsule_id: &CapsuleId) -> bool {
        let Some(stored) = self.claims.remove(capsule_id) else {
            return false;
        };
        self.remove_from_index(capsule_id, &stored.record.files);
        true
    }

    fn release_if_current(&mut self, capsule_id: &CapsuleId, token: u64) {
        let Some(stored) = self.claims.get(capsule_id) else {
            return;
        };
        if stored.token != token {
            return;
        }
        self.remove_claim(capsule_id);
    }

    fn refresh_if_current(&mut self, capsule_id: &CapsuleId, token: u64, additional: Duration) {
        let Some(stored) = self.claims.get_mut(capsule_id) else {
            return;
        };
        if stored.token != token {
            return;
        }

        let now = SystemTime::now();
        let base = if is_expired(stored.record.expires_at, now) {
            now
        } else {
            stored.record.expires_at
        };
        match base.checked_add(additional) {
            Some(expires_at) => stored.record.expires_at = expires_at,
            None => tracing::warn!(
                capsule_id = stored.record.capsule_id.as_str(),
                "claim refresh overflowed system time"
            ),
        }
    }

    fn conflict_warning(
        &self,
        requesting_capsule: &CapsuleId,
        files: &[PathBuf],
        now: SystemTime,
    ) -> Option<ConflictWarning> {
        let mut seen = BTreeSet::new();
        let mut conflicts = Vec::new();

        for file in files {
            let Some(holders) = self.by_file.get(file) else {
                continue;
            };
            for holding_capsule in holders {
                if holding_capsule == requesting_capsule {
                    continue;
                }
                if !seen.insert((file.clone(), holding_capsule.clone())) {
                    continue;
                }
                let Some(stored) = self.claims.get(holding_capsule) else {
                    tracing::warn!(
                        file = %file.display(),
                        capsule_id = holding_capsule.as_str(),
                        "claim index referenced missing capsule"
                    );
                    continue;
                };
                conflicts.push(ClaimConflict {
                    file: file.clone(),
                    holding_capsule: holding_capsule.clone(),
                    holding_since: elapsed_since(stored.record.claimed_at, now),
                    holding_kind: stored.record.kind,
                });
            }
        }

        if conflicts.is_empty() {
            None
        } else {
            Some(ConflictWarning {
                requesting_capsule: requesting_capsule.clone(),
                conflicts,
            })
        }
    }

    fn sweep_expired_at(&mut self, now: SystemTime) -> usize {
        let expired: Vec<_> = self
            .claims
            .iter()
            .filter(|(_, claim)| is_expired(claim.record.expires_at, now))
            .map(|(capsule_id, _)| capsule_id.clone())
            .collect();

        let removed = expired.len();
        for capsule_id in expired {
            self.remove_claim(&capsule_id);
        }
        removed
    }

    fn remove_from_index(&mut self, capsule_id: &CapsuleId, files: &[PathBuf]) {
        for file in files {
            let Some(holders) = self.by_file.get_mut(file) else {
                continue;
            };
            holders.retain(|holder| holder != capsule_id);
            if holders.is_empty() {
                self.by_file.remove(file);
            }
        }
    }
}

fn normalize_file_list(files: &[PathBuf], allow_empty: bool) -> Result<Vec<PathBuf>, ClaimError> {
    if files.is_empty() {
        return if allow_empty {
            Ok(Vec::new())
        } else {
            Err(ClaimError::InvalidPath(
                "claim requires at least one file".to_string(),
            ))
        };
    }

    let mut normalized = Vec::with_capacity(files.len());
    for file in files {
        normalized.push(normalize_claim_path(file)?);
    }
    normalized.sort();
    normalized.dedup();
    Ok(normalized)
}

fn normalize_claim_path(path: &Path) -> Result<PathBuf, ClaimError> {
    if path.as_os_str().is_empty() {
        return Err(ClaimError::InvalidPath("path is empty".to_string()));
    }
    if path.is_absolute() {
        return Err(ClaimError::InvalidPath(format!(
            "absolute paths are not claimable: {}",
            path.display()
        )));
    }

    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(segment) => normalized.push(segment),
            Component::CurDir => {}
            Component::ParentDir => {
                return Err(ClaimError::InvalidPath(format!(
                    "parent traversal is not claimable: {}",
                    path.display()
                )));
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(ClaimError::InvalidPath(format!(
                    "absolute paths are not claimable: {}",
                    path.display()
                )));
            }
        }
    }

    if normalized.as_os_str().is_empty() {
        return Err(ClaimError::InvalidPath(
            "path has no file segment".to_string(),
        ));
    }
    Ok(normalized)
}

fn checked_add(time: SystemTime, duration: Duration) -> Result<SystemTime, ClaimError> {
    time.checked_add(duration)
        .ok_or_else(|| ClaimError::InvalidPath("claim ttl overflows system time".to_string()))
}

fn is_expired(expires_at: SystemTime, now: SystemTime) -> bool {
    now.duration_since(expires_at).is_ok()
}

fn elapsed_since(start: SystemTime, now: SystemTime) -> Duration {
    now.duration_since(start).unwrap_or_default()
}
