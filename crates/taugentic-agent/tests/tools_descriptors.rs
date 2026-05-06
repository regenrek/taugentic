use ta_protocol::wire::ApprovalScope;
use taugentic_agent::tools::{ApplyPatchTool, Registry, ShellTool, Tool};

#[test]
fn mutating_tools_declare_approval_and_serial_execution_bits() {
    let shell = ShellTool.descriptor();
    assert_eq!(shell.approval_scope, Some(ApprovalScope::ProcessExec));
    assert!(!shell.read_only);
    assert!(!shell.parallel_safe);

    let apply_patch = ApplyPatchTool.descriptor();
    assert_eq!(apply_patch.approval_scope, Some(ApprovalScope::FileWrite));
    assert!(!apply_patch.read_only);
    assert!(!apply_patch.parallel_safe);
}

#[test]
fn all_builtins_register_readonly_and_mutating_tools() {
    let registry = Registry::with_all_builtins();
    let names = registry.iter().map(|(name, _)| name).collect::<Vec<_>>();
    assert_eq!(
        names,
        vec![
            "apply_patch",
            "list_directory",
            "read_file",
            "search",
            "shell"
        ]
    );
}
