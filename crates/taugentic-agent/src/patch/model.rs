use std::fmt::{self, Write};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Patch {
    pub operations: Vec<FileOp>,
}

impl Patch {
    pub fn to_patch_string(&self) -> String {
        let mut out = String::from("*** Begin Patch\n");
        for operation in &self.operations {
            write_operation(&mut out, operation);
        }
        out.push_str("*** End Patch\n");
        out
    }
}

impl fmt::Display for Patch {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.to_patch_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileOp {
    AddFile {
        path: PathBuf,
        contents: String,
    },
    DeleteFile {
        path: PathBuf,
    },
    UpdateFile {
        path: PathBuf,
        move_to: Option<PathBuf>,
        hunks: Vec<Hunk>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hunk {
    pub header: Option<String>,
    pub lines: Vec<HunkLine>,
    pub end_of_file: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HunkLine {
    pub kind: HunkLineKind,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HunkLineKind {
    Context,
    Added,
    Removed,
}

fn write_operation(out: &mut String, operation: &FileOp) {
    match operation {
        FileOp::AddFile { path, contents } => {
            let _ = writeln!(out, "*** Add File: {}", path.display());
            for line in contents.strip_suffix('\n').unwrap_or(contents).split('\n') {
                let _ = writeln!(out, "+{line}");
            }
        }
        FileOp::DeleteFile { path } => {
            let _ = writeln!(out, "*** Delete File: {}", path.display());
        }
        FileOp::UpdateFile {
            path,
            move_to,
            hunks,
        } => {
            let _ = writeln!(out, "*** Update File: {}", path.display());
            if let Some(move_to) = move_to {
                let _ = writeln!(out, "*** Move to: {}", move_to.display());
            }
            for hunk in hunks {
                match &hunk.header {
                    Some(header) => {
                        let _ = writeln!(out, "@@ {header}");
                    }
                    None => out.push_str("@@\n"),
                }
                for line in &hunk.lines {
                    let prefix = match line.kind {
                        HunkLineKind::Context => ' ',
                        HunkLineKind::Added => '+',
                        HunkLineKind::Removed => '-',
                    };
                    let _ = writeln!(out, "{prefix}{}", line.text);
                }
                if hunk.end_of_file {
                    out.push_str("*** End of File\n");
                }
            }
        }
    }
}
