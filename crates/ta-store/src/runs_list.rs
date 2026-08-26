use ta_protocol::wire::{
    NATIVE_RUN_LIST_MAX_LIMIT, RunHarnessKind, RunId, RunListFilter, RunSource, SessionId,
};

use crate::RunProjection;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeRunListQuery {
    pub session_id: SessionId,
    pub filter: RunListFilter,
    pub before: Option<NativeRunListCursor>,
    pub limit: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeRunListPage {
    pub runs: Vec<RunProjection>,
    pub next_cursor: Option<NativeRunListCursor>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeRunListCursor {
    pub started_at_ms: u64,
    pub run_id: RunId,
}

impl NativeRunListCursor {
    pub fn encode(&self) -> String {
        format!("{}:{}", self.started_at_ms, self.run_id.as_str())
    }

    pub fn decode(value: &str) -> Option<Self> {
        let (started_at_ms, run_id) = value.split_once(':')?;
        Some(Self {
            started_at_ms: started_at_ms.parse().ok()?,
            run_id: RunId::new(run_id).ok()?,
        })
    }
}

pub(crate) fn list_native_runs_from_projections(
    runs: impl IntoIterator<Item = RunProjection>,
    query: &NativeRunListQuery,
) -> NativeRunListPage {
    // AppService rejects out-of-range client limits; this clamp is a second
    // defense for direct repository callers and future store backends.
    let limit = query.limit.min(NATIVE_RUN_LIST_MAX_LIMIT as usize);
    let mut matches = runs
        .into_iter()
        .filter(|run| matches_native_run_query(run, query))
        .collect::<Vec<_>>();
    matches.sort_by(compare_native_runs_desc);

    let mut page = Vec::with_capacity(limit);
    let mut has_more = false;
    for run in matches {
        if query
            .before
            .as_ref()
            .is_some_and(|cursor| !is_before_cursor(&run, cursor))
        {
            continue;
        }
        if page.len() == limit {
            has_more = true;
            break;
        }
        page.push(run);
    }

    NativeRunListPage {
        next_cursor: has_more
            .then(|| page.last().map(native_run_cursor))
            .flatten(),
        runs: page,
    }
}

pub fn native_run_parent_id(run: &RunProjection) -> Option<RunId> {
    match &run.source {
        RunSource::NativeSubagent { parent_run_id, .. }
        | RunSource::Forked { parent_run_id, .. } => Some(parent_run_id.clone()),
        RunSource::User { .. } => None,
    }
}

fn matches_native_run_query(run: &RunProjection, query: &NativeRunListQuery) -> bool {
    if run.session_id != query.session_id || run.harness != RunHarnessKind::Native {
        return false;
    }
    if let Some(harnesses) = &query.filter.harness
        && !harnesses.is_empty()
        && !harnesses.contains(&run.harness)
    {
        return false;
    }
    if let Some(statuses) = &query.filter.status
        && !statuses.is_empty()
        && !statuses.contains(&run.status)
    {
        return false;
    }

    match &query.filter.parent_run_id {
        Some(parent_run_id) => native_run_parent_id(run).as_ref() == Some(parent_run_id),
        None => native_run_parent_id(run).is_none(),
    }
}

fn compare_native_runs_desc(left: &RunProjection, right: &RunProjection) -> std::cmp::Ordering {
    native_run_sort_started_at(right)
        .cmp(&native_run_sort_started_at(left))
        .then_with(|| right.id.as_str().cmp(left.id.as_str()))
}

fn is_before_cursor(run: &RunProjection, cursor: &NativeRunListCursor) -> bool {
    let started_at_ms = native_run_sort_started_at(run);
    started_at_ms < cursor.started_at_ms
        || (started_at_ms == cursor.started_at_ms && run.id.as_str() < cursor.run_id.as_str())
}

fn native_run_cursor(run: &RunProjection) -> NativeRunListCursor {
    NativeRunListCursor {
        started_at_ms: native_run_sort_started_at(run),
        run_id: run.id.clone(),
    }
}

fn native_run_sort_started_at(run: &RunProjection) -> u64 {
    run.started_at_ms.unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use ta_protocol::wire::{
        AgentStreamTurnId, NATIVE_RUN_LIST_MAX_LIMIT, RunStatus, RuntimeProfileId, SessionStatus,
    };

    use super::*;
    use crate::{InMemoryStore, ProjectionRepository, SessionProjection, StoreSeedRepository};

    #[test]
    fn list_native_runs_orders_filters_and_excludes_external_harnesses() {
        let session_id = session_id("session-a");
        let mut store = seeded_store(&session_id);
        seed_run(
            &mut store,
            run("run-old", &session_id, RunStatus::Completed, 100),
        );
        seed_run(
            &mut store,
            RunProjection {
                harness: RunHarnessKind::Acp,
                started_at_ms: Some(300),
                ..run("run-external", &session_id, RunStatus::Running, 300)
            },
        );
        seed_run(
            &mut store,
            run("run-new", &session_id, RunStatus::Running, 400),
        );
        seed_run(
            &mut store,
            run(
                "run-other-session",
                &make_session_id("session-b"),
                RunStatus::Running,
                500,
            ),
        );

        let page = store
            .list_native_runs(&NativeRunListQuery {
                session_id,
                filter: RunListFilter {
                    harness: Some(vec![RunHarnessKind::Native]),
                    status: Some(vec![RunStatus::Running]),
                    parent_run_id: None,
                },
                before: None,
                limit: 10,
            })
            .expect("native runs should list");

        assert_eq!(run_ids(&page.runs), vec!["run-new"]);
        assert_eq!(page.next_cursor, None);
    }

    #[test]
    fn list_native_runs_cursor_paginates_without_skip_or_duplicate() {
        let session_id = session_id("session-a");
        let mut store = seeded_store(&session_id);
        for run_id in ["run-a", "run-b", "run-c", "run-d"] {
            seed_run(
                &mut store,
                run(run_id, &session_id, RunStatus::Completed, 100),
            );
        }

        let first = store
            .list_native_runs(&NativeRunListQuery {
                session_id: session_id.clone(),
                filter: RunListFilter {
                    harness: None,
                    status: None,
                    parent_run_id: None,
                },
                before: None,
                limit: 2,
            })
            .expect("first page");
        let second = store
            .list_native_runs(&NativeRunListQuery {
                session_id,
                filter: RunListFilter {
                    harness: None,
                    status: None,
                    parent_run_id: None,
                },
                before: first.next_cursor.clone(),
                limit: 2,
            })
            .expect("second page");

        assert_eq!(run_ids(&first.runs), vec!["run-d", "run-c"]);
        assert_eq!(run_ids(&second.runs), vec!["run-b", "run-a"]);
        assert_eq!(second.next_cursor, None);
    }

    #[test]
    fn list_native_runs_returns_subagents_for_parent_filter_only() {
        let session_id = session_id("session-a");
        let parent_id = run_id("run-parent");
        let mut store = seeded_store(&session_id);
        seed_run(
            &mut store,
            run("run-parent", &session_id, RunStatus::Running, 300),
        );
        seed_run(
            &mut store,
            RunProjection {
                source: RunSource::NativeSubagent {
                    route: crate::default_test_run_source().route().clone(),
                    parent_run_id: parent_id.clone(),
                    parent_turn_id: AgentStreamTurnId::new("turn-parent").expect("turn id"),
                    output_contract: None,
                    model_id: None,
                    recipe_id: None,
                    workspace_scope: Default::default(),
                    cleanup_policy: Default::default(),
                    planned_write_files: Vec::new(),
                },
                ..run("run-child", &session_id, RunStatus::Completed, 400)
            },
        );

        let top_level = store
            .list_native_runs(&NativeRunListQuery {
                session_id: session_id.clone(),
                filter: RunListFilter {
                    harness: None,
                    status: None,
                    parent_run_id: None,
                },
                before: None,
                limit: 10,
            })
            .expect("top-level runs");
        let children = store
            .list_native_runs(&NativeRunListQuery {
                session_id,
                filter: RunListFilter {
                    harness: None,
                    status: None,
                    parent_run_id: Some(parent_id),
                },
                before: None,
                limit: 10,
            })
            .expect("child runs");

        assert_eq!(run_ids(&top_level.runs), vec!["run-parent"]);
        assert_eq!(run_ids(&children.runs), vec!["run-child"]);
        assert_eq!(
            native_run_parent_id(&children.runs[0])
                .as_ref()
                .map(|id| id.as_str()),
            Some("run-parent")
        );
    }

    #[test]
    fn list_native_runs_clamps_direct_store_limit_before_allocation() {
        let session_id = session_id("session-a");
        let mut store = seeded_store(&session_id);
        for index in 0..=NATIVE_RUN_LIST_MAX_LIMIT {
            seed_run(
                &mut store,
                run(
                    &format!("run-{index:03}"),
                    &session_id,
                    RunStatus::Completed,
                    u64::from(index),
                ),
            );
        }

        let page = store
            .list_native_runs(&NativeRunListQuery {
                session_id,
                filter: RunListFilter {
                    harness: None,
                    status: None,
                    parent_run_id: None,
                },
                before: None,
                limit: u32::MAX as usize,
            })
            .expect("native runs should list");

        assert_eq!(page.runs.len(), NATIVE_RUN_LIST_MAX_LIMIT as usize);
        assert!(page.runs.capacity() <= NATIVE_RUN_LIST_MAX_LIMIT as usize);
        assert!(page.next_cursor.is_some());
    }

    fn seeded_store(session_id: &SessionId) -> InMemoryStore {
        let mut store = InMemoryStore::current();
        store
            .save_session(SessionProjection {
                id: session_id.clone(),
                owner_client_name: "tests".to_string(),
                owner_principal_id: "principal-test-owner".to_string(),
                current_session_authority_hash: "authority".to_string(),
                current_session_authority_generation: 0,
                recovery_session_authority_hash: None,
                recovery_session_authority_generation: None,
                title: "Session".to_string(),
                status: SessionStatus::Idle,
                workspace_id: crate::default_test_workspace_id(),
            })
            .expect("session should seed");
        store
    }

    fn seed_run(store: &mut InMemoryStore, run: RunProjection) {
        store.save_run(run).expect("run should seed");
    }

    fn run(
        id: &str,
        session_id: &SessionId,
        status: RunStatus,
        started_at_ms: u64,
    ) -> RunProjection {
        RunProjection {
            id: run_id(id),
            session_id: session_id.clone(),
            runtime_profile_id: RuntimeProfileId::new("runtime-openai-safe")
                .expect("runtime profile id"),
            objective: format!("Objective {id}"),
            status,
            harness: RunHarnessKind::Native,
            source: crate::default_test_run_source(),
            execution_context: crate::default_test_execution_context(),
            result: None,
            contract_violation: None,
            started_at_ms: Some(started_at_ms),
            ended_at_ms: None,
            last_event_seq: Some(started_at_ms / 10),
            workspace_info: None,
            claimed_files: Vec::new(),
            conflict_summary: None,
        }
    }

    fn run_ids(runs: &[RunProjection]) -> Vec<&str> {
        runs.iter().map(|run| run.id.as_str()).collect()
    }

    fn session_id(value: &str) -> SessionId {
        SessionId::new(value).expect("session id")
    }

    fn make_session_id(value: &str) -> SessionId {
        SessionId::new(value).expect("session id")
    }

    fn run_id(value: &str) -> RunId {
        RunId::new(value).expect("run id")
    }
}
