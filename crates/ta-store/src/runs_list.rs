use std::collections::{BTreeMap, BTreeSet};
use ta_protocol::wire::{
    NATIVE_RUN_LIST_MAX_LIMIT, NativeRunRelationship, RUN_LINEAGE_GRAPH_MAX_BYTES,
    RUN_LINEAGE_GRAPH_MAX_EDGES, RUN_LINEAGE_GRAPH_MAX_NODES, RunId, RunLineageGraphEdge,
    RunLineageGraphResult, RunListEntry, RunListFilter, RunSource, SessionId,
};

use crate::RunProjection;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeRunListQuery {
    pub session_id: SessionId,
    pub filter: RunListFilter,
    /// Native workspace views ask for the whole daemon-owned relationship
    /// projection; direct repository callers can still request roots only.
    pub include_children: bool,
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
        | RunSource::FreshSpawn { parent_run_id, .. }
        | RunSource::Forked { parent_run_id, .. }
        | RunSource::AccountSwitchedContinuation { parent_run_id, .. } => {
            Some(parent_run_id.clone())
        }
        RunSource::ScheduledWork { .. } | RunSource::User { .. } => None,
    }
}

/// One daemon/store-owned, parent-closed graph. Sorting and cap application are
/// deterministic; presentation never has to walk or repair lineage.
pub fn run_lineage_graph_from_projections(
    runs: impl IntoIterator<Item = RunProjection>,
    session_id: &SessionId,
) -> RunLineageGraphResult {
    let mut all = runs
        .into_iter()
        .filter(|run| &run.session_id == session_id)
        .collect::<Vec<_>>();
    all.sort_by(graph_priority);
    let total_count = all.len() as u32;
    let by_id = all
        .iter()
        .cloned()
        .map(|run| (run.id.clone(), run))
        .collect::<BTreeMap<_, _>>();
    let mut included = BTreeSet::new();
    for run in &all {
        let mut chain = Vec::new();
        let mut cursor = Some(run.id.clone());
        let mut seen = BTreeSet::new();
        while let Some(current) = cursor {
            if !seen.insert(current.clone()) {
                break;
            }
            if !included.contains(&current) {
                chain.push(current.clone());
            }
            cursor = by_id
                .get(&current)
                .and_then(native_run_parent_id)
                .filter(|parent| by_id.contains_key(parent));
        }
        if included.len() + chain.len() > RUN_LINEAGE_GRAPH_MAX_NODES as usize {
            continue;
        }
        included.extend(chain);
    }
    let mut cycle_broken = false;
    let mut edges = Vec::new();
    let mut orphan_run_ids = Vec::new();
    for id in &included {
        let run = &by_id[id];
        let Some(parent) = native_run_parent_id(run) else {
            continue;
        };
        if !by_id.contains_key(&parent) {
            orphan_run_ids.push(id.clone());
            continue;
        }
        if included.contains(&parent) {
            // The edge into the lexicographically smallest node in a cycle is
            // always omitted, yielding one reproducible rooted tree.
            if closes_cycle(&by_id, id, &parent) && id.as_str() <= parent.as_str() {
                cycle_broken = true;
                continue;
            }
            if edges.len() < RUN_LINEAGE_GRAPH_MAX_EDGES as usize {
                edges.push(RunLineageGraphEdge {
                    parent_run_id: parent,
                    child_run_id: id.clone(),
                });
            }
        }
    }
    let nodes = included
        .iter()
        .map(|id| run_list_entry(&by_id[id]))
        .collect::<Vec<_>>();
    let mut result = RunLineageGraphResult {
        nodes,
        edges,
        orphan_run_ids,
        total_count,
        omitted_count: 0,
        truncated: false,
        cycle_broken,
    };
    // Result size is part of the daemon contract, not a desktop rendering concern.
    while serde_json::to_vec(&result).map_or(true, |bytes| {
        bytes.len() > RUN_LINEAGE_GRAPH_MAX_BYTES as usize
    }) {
        let children = result
            .edges
            .iter()
            .map(|edge| edge.parent_run_id.clone())
            .collect::<BTreeSet<_>>();
        let Some(index) = result
            .nodes
            .iter()
            .rposition(|node| !children.contains(&node.id))
        else {
            break;
        };
        let removed = result.nodes.remove(index).id;
        result
            .edges
            .retain(|edge| edge.parent_run_id != removed && edge.child_run_id != removed);
        result.orphan_run_ids.retain(|id| id != &removed);
    }
    result.omitted_count = total_count.saturating_sub(result.nodes.len() as u32);
    result.truncated = result.omitted_count > 0;
    result
}

fn graph_priority(left: &RunProjection, right: &RunProjection) -> std::cmp::Ordering {
    let left_active = matches!(
        left.status,
        ta_protocol::wire::RunStatus::Queued
            | ta_protocol::wire::RunStatus::Running
            | ta_protocol::wire::RunStatus::WaitingForApproval
    );
    let right_active = matches!(
        right.status,
        ta_protocol::wire::RunStatus::Queued
            | ta_protocol::wire::RunStatus::Running
            | ta_protocol::wire::RunStatus::WaitingForApproval
    );
    right_active
        .cmp(&left_active)
        .then_with(|| native_run_sort_started_at(right).cmp(&native_run_sort_started_at(left)))
        .then_with(|| left.id.as_str().cmp(right.id.as_str()))
}

fn closes_cycle(by_id: &BTreeMap<RunId, RunProjection>, child: &RunId, parent: &RunId) -> bool {
    let mut cursor = Some(parent.clone());
    let mut seen = BTreeSet::new();
    while let Some(id) = cursor {
        if !seen.insert(id.clone()) {
            return false;
        }
        if &id == child {
            return true;
        }
        cursor = by_id.get(&id).and_then(native_run_parent_id);
    }
    false
}

fn relationship(run: &RunProjection) -> NativeRunRelationship {
    match &run.source {
        RunSource::ScheduledWork { .. } | RunSource::User { .. } => NativeRunRelationship::Root,
        RunSource::NativeSubagent { parent_run_id, .. } => NativeRunRelationship::NativeSubagent {
            parent_run_id: parent_run_id.clone(),
        },
        RunSource::FreshSpawn { parent_run_id, .. } => NativeRunRelationship::FreshSpawn {
            parent_run_id: parent_run_id.clone(),
        },
        RunSource::Forked {
            parent_run_id,
            parent_event_seq,
            ..
        } => NativeRunRelationship::Fork {
            parent_run_id: parent_run_id.clone(),
            parent_event_seq: *parent_event_seq,
        },
        RunSource::AccountSwitchedContinuation {
            parent_run_id,
            parent_event_seq,
            ..
        } => NativeRunRelationship::AccountSwitchedContinuation {
            parent_run_id: parent_run_id.clone(),
            parent_event_seq: *parent_event_seq,
        },
    }
}

fn run_list_entry(run: &RunProjection) -> RunListEntry {
    let (output_contract, recipe_id) = match &run.source {
        RunSource::NativeSubagent {
            output_contract,
            recipe_id,
            ..
        }
        | RunSource::FreshSpawn {
            output_contract,
            recipe_id,
            ..
        }
        | RunSource::User {
            output_contract,
            recipe_id,
            ..
        } => (*output_contract, recipe_id.clone()),
        RunSource::ScheduledWork { .. }
        | RunSource::Forked { .. }
        | RunSource::AccountSwitchedContinuation { .. } => (None, None),
    };
    RunListEntry {
        id: run.id.clone(),
        relationship: relationship(run),
        output_contract,
        recipe_id,
        harness: run.harness,
        status: run.status,
        started_at_ms: run.started_at_ms,
        ended_at_ms: run.ended_at_ms,
        last_event_seq: run.last_event_seq,
        objective_preview: Some(run.objective.trim().chars().take(120).collect()),
        workspace_info: run.workspace_info.clone(),
        claimed_files: run.claimed_files.clone(),
        conflict_summary: run.conflict_summary.clone(),
    }
}

fn matches_native_run_query(run: &RunProjection, query: &NativeRunListQuery) -> bool {
    if run.session_id != query.session_id {
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
        None => query.include_children || native_run_parent_id(run).is_none(),
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
        AgentStreamTurnId, NATIVE_RUN_LIST_MAX_LIMIT, RunHarnessKind, RunStatus, RuntimeProfileId,
        SessionStatus,
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
                include_children: false,
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
                include_children: false,
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
                include_children: false,
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
                include_children: false,
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
                include_children: false,
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
    fn fresh_spawn_relationship_rows_include_generic_harness_children() {
        let session_id = session_id("session-fresh");
        let parent_id = run_id("run-parent-fresh");
        let mut store = seeded_store(&session_id);
        seed_run(
            &mut store,
            RunProjection {
                harness: RunHarnessKind::CodexAppServer,
                source: RunSource::FreshSpawn {
                    route: crate::default_test_run_source().route().clone(),
                    parent_run_id: parent_id.clone(),
                    output_contract: None,
                    model_id: None,
                    recipe_id: None,
                    workspace_scope: Default::default(),
                    cleanup_policy: Default::default(),
                    planned_write_files: Vec::new(),
                },
                ..run("run-fresh-codex", &session_id, RunStatus::Queued, 400)
            },
        );

        let page = store
            .list_native_runs(&NativeRunListQuery {
                session_id,
                filter: RunListFilter {
                    harness: None,
                    status: None,
                    parent_run_id: Some(parent_id),
                },
                include_children: true,
                before: None,
                limit: 10,
            })
            .expect("fresh relationship row should list");

        assert_eq!(run_ids(&page.runs), vec!["run-fresh-codex"]);
        assert_eq!(page.runs[0].harness, RunHarnessKind::CodexAppServer);
        assert!(matches!(page.runs[0].source, RunSource::FreshSpawn { .. }));
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
                include_children: false,
                before: None,
                limit: u32::MAX as usize,
            })
            .expect("native runs should list");

        assert_eq!(page.runs.len(), NATIVE_RUN_LIST_MAX_LIMIT as usize);
        assert!(page.runs.capacity() <= NATIVE_RUN_LIST_MAX_LIMIT as usize);
        assert!(page.next_cursor.is_some());
    }

    #[test]
    fn lineage_graph_is_parent_closed_and_preserves_every_relationship_kind() {
        let session_id = session_id("session-lineage");
        let root_id = run_id("root");
        let mut runs = vec![run("root", &session_id, RunStatus::Completed, 10)];
        let route = crate::default_test_run_source().route().clone();
        runs.push(RunProjection {
            source: RunSource::NativeSubagent {
                route: route.clone(),
                parent_run_id: root_id.clone(),
                parent_turn_id: AgentStreamTurnId::new("turn-root").expect("turn"),
                output_contract: None,
                model_id: None,
                recipe_id: None,
                workspace_scope: Default::default(),
                cleanup_policy: Default::default(),
                planned_write_files: Vec::new(),
            },
            ..run("subagent", &session_id, RunStatus::Completed, 20)
        });
        runs.push(RunProjection {
            source: RunSource::FreshSpawn {
                route: route.clone(),
                parent_run_id: root_id.clone(),
                output_contract: None,
                model_id: None,
                recipe_id: None,
                workspace_scope: Default::default(),
                cleanup_policy: Default::default(),
                planned_write_files: Vec::new(),
            },
            ..run("fresh", &session_id, RunStatus::Completed, 30)
        });
        runs.push(RunProjection {
            source: RunSource::Forked {
                route,
                parent_run_id: root_id.clone(),
                parent_event_seq: 7,
            },
            ..run("fork", &session_id, RunStatus::Completed, 40)
        });
        runs.push(RunProjection {
            source: RunSource::AccountSwitchedContinuation {
                route: crate::default_test_run_source().route().clone(),
                parent_run_id: root_id.clone(),
                parent_event_seq: 9,
            },
            ..run("account-switch", &session_id, RunStatus::Completed, 50)
        });

        let graph = run_lineage_graph_from_projections(runs, &session_id);
        assert_eq!(graph.nodes.len(), 5);
        assert_eq!(graph.edges.len(), 4);
        assert!(
            graph
                .nodes
                .iter()
                .any(|node| matches!(node.relationship, NativeRunRelationship::Root))
        );
        assert!(graph.nodes.iter().any(|node| matches!(
            node.relationship,
            NativeRunRelationship::NativeSubagent { .. }
        )));
        assert!(
            graph
                .nodes
                .iter()
                .any(|node| matches!(node.relationship, NativeRunRelationship::FreshSpawn { .. }))
        );
        assert!(graph.nodes.iter().any(|node| matches!(
            node.relationship,
            NativeRunRelationship::Fork {
                parent_event_seq: 7,
                ..
            }
        )));
        assert!(graph.nodes.iter().any(|node| matches!(
            node.relationship,
            NativeRunRelationship::AccountSwitchedContinuation {
                parent_event_seq: 9,
                ..
            }
        )));
        assert!(graph.edges.iter().all(|edge| edge.parent_run_id == root_id));
    }

    #[test]
    fn lineage_graph_prioritizes_active_parent_closed_chains_and_enforces_caps() {
        let session_id = session_id("session-priority");
        let parent_id = run_id("root-old");
        let route = crate::default_test_run_source().route().clone();
        let mut runs = vec![
            run("root-old", &session_id, RunStatus::Completed, 1),
            RunProjection {
                source: RunSource::FreshSpawn {
                    route,
                    parent_run_id: parent_id.clone(),
                    output_contract: None,
                    model_id: None,
                    recipe_id: None,
                    workspace_scope: Default::default(),
                    cleanup_policy: Default::default(),
                    planned_write_files: Vec::new(),
                },
                ..run("active-child", &session_id, RunStatus::Running, 2)
            },
        ];
        for index in 0..127 {
            runs.push(run(
                &format!("terminal-{index:03}"),
                &session_id,
                RunStatus::Completed,
                100 + index,
            ));
        }

        let graph = run_lineage_graph_from_projections(runs, &session_id);
        assert_eq!(graph.nodes.len(), RUN_LINEAGE_GRAPH_MAX_NODES as usize);
        assert!(graph.truncated);
        assert!(
            graph
                .nodes
                .iter()
                .any(|node| node.id.as_str() == "active-child")
        );
        assert!(graph.nodes.iter().any(|node| node.id == parent_id));
        assert!(
            graph
                .edges
                .iter()
                .any(|edge| edge.parent_run_id == parent_id
                    && edge.child_run_id.as_str() == "active-child")
        );
        assert!(graph.edges.len() <= RUN_LINEAGE_GRAPH_MAX_EDGES as usize);
        assert!(
            serde_json::to_vec(&graph).expect("graph JSON").len()
                <= RUN_LINEAGE_GRAPH_MAX_BYTES as usize
        );
    }

    #[test]
    fn lineage_graph_reports_orphans_breaks_cycles_and_never_exceeds_json_cap() {
        let session_id = session_id("session-safety");
        let route = crate::default_test_run_source().route().clone();
        let runs = vec![
            RunProjection {
                source: RunSource::FreshSpawn {
                    route: route.clone(),
                    parent_run_id: run_id("missing"),
                    output_contract: None,
                    model_id: None,
                    recipe_id: None,
                    workspace_scope: Default::default(),
                    cleanup_policy: Default::default(),
                    planned_write_files: Vec::new(),
                },
                ..run("orphan", &session_id, RunStatus::Completed, 1)
            },
            RunProjection {
                source: RunSource::Forked {
                    route: route.clone(),
                    parent_run_id: run_id("cycle-b"),
                    parent_event_seq: 1,
                },
                ..run("cycle-a", &session_id, RunStatus::Completed, 2)
            },
            RunProjection {
                source: RunSource::Forked {
                    route,
                    parent_run_id: run_id("cycle-a"),
                    parent_event_seq: 2,
                },
                ..run("cycle-b", &session_id, RunStatus::Completed, 3)
            },
        ];

        let graph = run_lineage_graph_from_projections(runs, &session_id);
        assert_eq!(graph.orphan_run_ids, vec![run_id("orphan")]);
        assert!(graph.cycle_broken);
        assert!(graph.edges.len() < 2);
        assert!(
            serde_json::to_vec(&graph).expect("graph JSON").len()
                <= RUN_LINEAGE_GRAPH_MAX_BYTES as usize
        );
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
                next_run_selection: ta_protocol::wire::SessionNextRunSelection::Unselected,
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
