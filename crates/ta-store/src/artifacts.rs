use serde::{Deserialize, Serialize};
use ta_protocol::wire::{
    ArtifactId, ArtifactKind, ArtifactMetadata, ArtifactSummary, RunId, SessionId,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactRecord {
    pub id: ArtifactId,
    pub session_id: SessionId,
    pub run_id: RunId,
    pub kind: ArtifactKind,
    pub metadata: ArtifactMetadata,
    pub storage_path: String,
}

impl ArtifactRecord {
    /// The protocol has exactly one metadata shape for each durable artifact
    /// kind. Keep that invariant at the store boundary so seeded fixtures and
    /// production commits cannot create an artifact that downstream consumers
    /// would have to reinterpret.
    pub fn validate_metadata(&self) -> Result<(), crate::StoreError> {
        match (self.kind, &self.metadata) {
            (ArtifactKind::Image, ArtifactMetadata::Image(_))
            | (
                ArtifactKind::Transcript
                | ArtifactKind::Patch
                | ArtifactKind::FileSnapshot
                | ArtifactKind::CommandLog,
                ArtifactMetadata::Standard,
            ) => Ok(()),
            (kind, metadata) => Err(crate::StoreError::ArtifactMetadataMismatch {
                kind: format!("{kind:?}"),
                metadata: format!("{metadata:?}"),
            }),
        }
    }
}

pub fn project_artifact_summary(artifact: &ArtifactRecord) -> ArtifactSummary {
    let fallback = match artifact.kind {
        ArtifactKind::Transcript => "transcript",
        ArtifactKind::Patch => "patch",
        ArtifactKind::FileSnapshot => "file snapshot",
        ArtifactKind::CommandLog => "command log",
        ArtifactKind::Image => "image",
    };
    let display_name = std::path::Path::new(&artifact.storage_path)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .map(|name| {
            name.chars()
                .take(160)
                .map(|character| {
                    if character.is_control() {
                        '�'
                    } else {
                        character
                    }
                })
                .collect::<String>()
        })
        .unwrap_or_else(|| fallback.to_string());
    ArtifactSummary {
        id: artifact.id.clone(),
        run_id: artifact.run_id.clone(),
        kind: artifact.kind,
        metadata: artifact.metadata.clone(),
        display_name,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn artifact_summary_exposes_only_a_bounded_safe_display_name() {
        let artifact = ArtifactRecord {
            id: ArtifactId::new("artifact-safe-name").expect("artifact id"),
            session_id: SessionId::new("session-safe-name").expect("session id"),
            run_id: RunId::new("run-safe-name").expect("run id"),
            kind: ArtifactKind::Patch,
            metadata: ArtifactMetadata::Standard,
            storage_path: format!("internal/private/bad\n-{}.diff", "x".repeat(180)),
        };

        let summary = project_artifact_summary(&artifact);

        assert_eq!(summary.display_name.chars().count(), 160);
        assert!(summary.display_name.ends_with('x'));
        assert!(summary.display_name.contains('�'));
        assert!(!summary.display_name.contains("internal"));
        assert!(!summary.display_name.chars().any(char::is_control));
        assert_eq!(
            artifact.storage_path,
            format!("internal/private/bad\n-{}.diff", "x".repeat(180))
        );
    }

    #[test]
    fn image_artifact_metadata_must_match_the_artifact_kind() {
        let artifact = ArtifactRecord {
            id: ArtifactId::new("artifact-image").expect("artifact id"),
            session_id: SessionId::new("session-image").expect("session id"),
            run_id: RunId::new("run-image").expect("run id"),
            kind: ArtifactKind::Image,
            metadata: ArtifactMetadata::Standard,
            storage_path: "image.png".to_string(),
        };
        assert!(matches!(
            artifact.validate_metadata(),
            Err(crate::StoreError::ArtifactMetadataMismatch { .. })
        ));
    }
}
