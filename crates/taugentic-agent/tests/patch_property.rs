use proptest::prelude::*;
use taugentic_agent::patch::parse_patch;

proptest! {
    #[test]
    fn parse_serialize_parse_is_stable(ops in prop::collection::vec(file_op(), 1..8)) {
        let patch = format!("*** Begin Patch\n{}*** End Patch\n", ops.join(""));
        let parsed = parse_patch(&patch)?;
        let serialized = parsed.to_patch_string();
        let reparsed = parse_patch(&serialized)?;
        prop_assert_eq!(parsed, reparsed);
    }
}

fn file_op() -> impl Strategy<Value = String> {
    prop_oneof![
        (path(), lines(1..4)).prop_map(|(path, lines)| {
            format!(
                "*** Add File: {path}\n{}",
                lines
                    .into_iter()
                    .map(|line| format!("+{line}\n"))
                    .collect::<String>()
            )
        }),
        path().prop_map(|path| format!("*** Delete File: {path}\n")),
        (path(), prop::option::of(path()), hunk_lines()).prop_map(|(path, move_to, lines)| {
            let move_line = move_to
                .map(|path| format!("*** Move to: {path}\n"))
                .unwrap_or_default();
            format!("*** Update File: {path}\n{move_line}@@\n{lines}")
        }),
    ]
}

fn path() -> impl Strategy<Value = String> {
    "[a-z][a-z0-9_]{0,8}(/[a-z][a-z0-9_]{0,8}){0,2}\\.txt"
}

fn lines(range: std::ops::Range<usize>) -> impl Strategy<Value = Vec<String>> {
    prop::collection::vec("[a-zA-Z0-9 _.-]{0,24}", range)
}

fn hunk_lines() -> impl Strategy<Value = String> {
    lines(1..5).prop_map(|lines| {
        lines
            .into_iter()
            .enumerate()
            .map(|(index, line)| {
                let prefix = match index % 3 {
                    0 => ' ',
                    1 => '-',
                    _ => '+',
                };
                format!("{prefix}{line}\n")
            })
            .collect::<String>()
    })
}
