use std::fs;

use ta_protocol::wire::RunId;
use taugentic_agent::artifacts::ArtifactWriter;

#[cfg(unix)]
#[test]
fn artifact_path_containment_rejects_run_dir_symlink_escape() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().join("artifacts");
    let outside = temp.path().join("outside");
    fs::create_dir_all(&root).expect("root");
    fs::create_dir_all(&outside).expect("outside");
    let run_id = RunId::new("run-symlink-escape").expect("run id");
    std::os::unix::fs::symlink(&outside, root.join(run_id.as_str())).expect("symlink");
    let writer = ArtifactWriter::new(&root, run_id).expect("writer");

    let error = writer
        .write_patch("patch", "diff --git a/secret b/secret")
        .expect_err("symlink escape should be rejected");

    assert!(error.to_string().contains("must stay inside artifact root"));
    assert!(
        fs::read_dir(&outside)
            .expect("outside dir")
            .next()
            .is_none(),
        "artifact writer leaked outside root"
    );
}

#[test]
fn artifact_path_containment_rejects_parent_component_run_id_before_creating_escape() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().join("artifacts");
    let outside = temp.path().join("outside");
    let run_id = RunId::new("../outside").expect("run id accepts arbitrary strings");

    let error = match ArtifactWriter::new(&root, run_id) {
        Ok(_) => panic!("run id escape should be rejected"),
        Err(error) => error,
    };

    assert!(error.to_string().contains("artifact run id"));
    assert!(
        !outside.exists(),
        "artifact writer must reject parent components before creating outside directories"
    );
}
