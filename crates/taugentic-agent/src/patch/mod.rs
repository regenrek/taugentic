pub mod apply;
pub mod model;
pub mod parser;
pub mod writer;

pub use apply::{AppliedPatch, ApplyError, FileChange, FileChangeKind, apply_patch};
pub use model::{FileOp, Hunk, HunkLine, HunkLineKind, Patch};
pub use parser::{ParseError, parse_patch};
pub use writer::{WriteError, WriteReport, write_applied_patch};
