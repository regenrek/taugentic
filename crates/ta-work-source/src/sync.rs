use ta_protocol::wire::{SourceCursor, WorkItem};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FetchOutcome {
    Items {
        items: Vec<WorkItem>,
        cursor: SourceCursor,
    },
    NotModified {
        cursor: SourceCursor,
    },
}
