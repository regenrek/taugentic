use ta_protocol::wire::{
    BoundedFileContent, WORKSPACE_FILE_ATTACHMENT_MAX_COUNT, WORKSPACE_TEXT_MAX_BYTES,
    WorkspaceFileAttachmentRequest, WorkspaceFileOpenExternalParams, WorkspaceFileReadParams,
    WorkspaceFileTreeParams, WorkspaceFileWriteParams, WorkspacePath,
};

use super::*;

fn open_project_fixture(
    service: &AppService,
    root: &std::path::Path,
) -> (crate::ProjectId, crate::WorkspaceId) {
    let path = WorkspacePath::canonicalize_existing(root).expect("workspace path");
    let (project_id, snapshot) = service
        .open_project(TEST_OWNER_PRINCIPAL_ID, path, true)
        .expect("project should open");
    let workspace_id = snapshot
        .projects
        .iter()
        .find(|project| project.id == project_id)
        .and_then(|project| project.workspace_ids.first())
        .cloned()
        .expect("project workspace");
    (project_id, workspace_id)
}

#[test]
fn workspace_file_tree_and_reads_are_bounded_typed_and_project_scoped() {
    let service = AppService::bootstrap().expect("service should bootstrap");
    let root = tempfile::tempdir().expect("workspace tempdir");
    std::fs::create_dir_all(root.path().join("src")).expect("src should create");
    std::fs::create_dir_all(root.path().join("node_modules/ignored"))
        .expect("node_modules should create");
    std::fs::write(root.path().join("src/main.rs"), "fn main() {}\n").expect("source should write");
    std::fs::write(root.path().join("node_modules/ignored/index.js"), "ignored")
        .expect("ignored file should write");
    std::fs::write(root.path().join("pixel.png"), b"\x89PNG\r\n\x1a\nminimal")
        .expect("image should write");
    std::fs::write(root.path().join("report.pdf"), minimal_pdf()).expect("pdf should write");
    let (project_id, workspace_id) = open_project_fixture(&service, root.path());

    let tree = service
        .workspace_file_tree(
            TEST_OWNER_PRINCIPAL_ID,
            &WorkspaceFileTreeParams {
                project_id: project_id.clone(),
                workspace_id: workspace_id.clone(),
            },
        )
        .expect("tree should load");
    assert!(tree.entries.iter().any(|entry| entry.path == "src/main.rs"));
    assert!(
        tree.entries
            .iter()
            .all(|entry| !entry.path.starts_with("node_modules"))
    );
    assert!(!tree.truncated);

    let source = service
        .read_workspace_file(
            TEST_OWNER_PRINCIPAL_ID,
            &WorkspaceFileReadParams {
                project_id: project_id.clone(),
                workspace_id: workspace_id.clone(),
                path: "src/main.rs".to_string(),
                pdf_page_index: None,
            },
        )
        .expect("source should read");
    assert!(matches!(
        source.content,
        BoundedFileContent::Text { text, language, .. }
            if text == "fn main() {}\n" && language.as_deref() == Some("rust")
    ));

    let image = service
        .read_workspace_file(
            TEST_OWNER_PRINCIPAL_ID,
            &WorkspaceFileReadParams {
                project_id: project_id.clone(),
                workspace_id: workspace_id.clone(),
                path: "pixel.png".to_string(),
                pdf_page_index: None,
            },
        )
        .expect("image should read");
    assert!(matches!(
        image.content,
        BoundedFileContent::Image { data_uri, media_type, .. }
            if data_uri.starts_with("data:image/png;base64,") && media_type == "image/png"
    ));

    let pdf = service
        .read_workspace_file(
            TEST_OWNER_PRINCIPAL_ID,
            &WorkspaceFileReadParams {
                project_id: project_id.clone(),
                workspace_id: workspace_id.clone(),
                path: "report.pdf".to_string(),
                pdf_page_index: None,
            },
        )
        .expect("pdf should read");
    assert!(matches!(
        pdf.content,
        BoundedFileContent::Pdf {
            preview_data_uri,
            page_index: 0,
            page_count: 1,
            ..
        } if preview_data_uri.starts_with("data:image/png;base64,")
    ));
    let missing_pdf_page = service
        .read_workspace_file(
            TEST_OWNER_PRINCIPAL_ID,
            &WorkspaceFileReadParams {
                project_id: project_id.clone(),
                workspace_id: workspace_id.clone(),
                path: "report.pdf".to_string(),
                pdf_page_index: Some(1),
            },
        )
        .expect_err("out-of-range PDF page should fail");
    assert!(matches!(
        missing_pdf_page,
        AppServiceError::WorkspaceFilePdfPageOutOfRange {
            page_index: 1,
            page_count: 1,
            ..
        }
    ));

    let external = service
        .workspace_file_open_external(
            TEST_OWNER_PRINCIPAL_ID,
            &WorkspaceFileOpenExternalParams {
                project_id: project_id.clone(),
                workspace_id: workspace_id.clone(),
                path: "src/main.rs".to_string(),
            },
        )
        .expect("external path should validate");
    assert_eq!(external.path.as_path(), root.path().join("src/main.rs"));

    let error = service
        .read_workspace_file(
            OTHER_TEST_OWNER_PRINCIPAL_ID,
            &WorkspaceFileReadParams {
                project_id,
                workspace_id,
                path: "src/main.rs".to_string(),
                pdf_page_index: None,
            },
        )
        .expect_err("another principal must not read this project");
    assert!(matches!(error, AppServiceError::ProjectNotFound(_)));
}

#[test]
fn workspace_text_save_requires_approval_and_exact_revision() {
    let service = AppService::bootstrap().expect("service should bootstrap");
    let root = tempfile::tempdir().expect("workspace tempdir");
    std::fs::write(root.path().join("notes.txt"), "before\n").expect("notes should write");
    let (project_id, workspace_id) = open_project_fixture(&service, root.path());
    let read = service
        .read_workspace_file(
            TEST_OWNER_PRINCIPAL_ID,
            &WorkspaceFileReadParams {
                project_id: project_id.clone(),
                workspace_id: workspace_id.clone(),
                path: "notes.txt".to_string(),
                pdf_page_index: None,
            },
        )
        .expect("notes should read");
    let revision = read.content.revision().to_string();
    let params = WorkspaceFileWriteParams {
        project_id,
        workspace_id,
        path: "notes.txt".to_string(),
        expected_revision: revision.clone(),
        text: "after\n".to_string(),
        user_approved: false,
    };

    let error = service
        .write_workspace_file(TEST_OWNER_PRINCIPAL_ID, &params)
        .expect_err("unapproved save should fail");
    assert!(matches!(
        error,
        AppServiceError::WorkspaceFileWriteApprovalRequired
    ));

    let saved = service
        .write_workspace_file(
            TEST_OWNER_PRINCIPAL_ID,
            &WorkspaceFileWriteParams {
                user_approved: true,
                ..params.clone()
            },
        )
        .expect("approved save should succeed");
    assert_ne!(saved.revision, revision);
    assert_eq!(
        std::fs::read_to_string(root.path().join("notes.txt")).expect("saved notes"),
        "after\n"
    );

    let error = service
        .write_workspace_file(
            TEST_OWNER_PRINCIPAL_ID,
            &WorkspaceFileWriteParams {
                user_approved: true,
                text: "stale write\n".to_string(),
                ..params
            },
        )
        .expect_err("stale save should fail");
    assert!(matches!(error, AppServiceError::WorkspaceFileStale(_)));
}

#[test]
fn workspace_file_access_rejects_traversal_symlinks_invalid_kinds_and_oversize_text() {
    let service = AppService::bootstrap().expect("service should bootstrap");
    let root = tempfile::tempdir().expect("workspace tempdir");
    let outside = tempfile::tempdir().expect("outside tempdir");
    std::fs::write(outside.path().join("outside.txt"), "outside")
        .expect("outside file should write");
    std::fs::write(root.path().join("invalid.png"), "not a png")
        .expect("invalid image should write");
    std::fs::write(
        root.path().join("oversize.txt"),
        vec![b'x'; WORKSPACE_TEXT_MAX_BYTES as usize + 1],
    )
    .expect("oversize text should write");
    #[cfg(unix)]
    std::os::unix::fs::symlink(
        outside.path().join("outside.txt"),
        root.path().join("linked.txt"),
    )
    .expect("symlink should create");
    let (project_id, workspace_id) = open_project_fixture(&service, root.path());

    for (path, expected) in [
        ("../outside.txt", "WorkspaceFileInvalidPath"),
        ("invalid.png", "WorkspaceFileUnsupportedKind"),
        ("oversize.txt", "WorkspaceFileTooLarge"),
    ] {
        let error = service
            .read_workspace_file(
                TEST_OWNER_PRINCIPAL_ID,
                &WorkspaceFileReadParams {
                    project_id: project_id.clone(),
                    workspace_id: workspace_id.clone(),
                    path: path.to_string(),
                    pdf_page_index: None,
                },
            )
            .expect_err("unsafe or invalid read should fail");
        assert!(
            format!("{error:?}").starts_with(expected),
            "expected {expected}, got {error:?}"
        );
    }

    #[cfg(unix)]
    {
        let tree = service
            .workspace_file_tree(
                TEST_OWNER_PRINCIPAL_ID,
                &WorkspaceFileTreeParams {
                    project_id: project_id.clone(),
                    workspace_id: workspace_id.clone(),
                },
            )
            .expect("tree should report a symlink without following it");
        assert!(
            tree.entries
                .iter()
                .any(|entry| entry.path == "linked.txt" && entry.is_symlink)
        );
        let error = service
            .read_workspace_file(
                TEST_OWNER_PRINCIPAL_ID,
                &WorkspaceFileReadParams {
                    project_id,
                    workspace_id,
                    path: "linked.txt".to_string(),
                    pdf_page_index: None,
                },
            )
            .expect_err("symlink read should fail");
        assert!(matches!(
            error,
            AppServiceError::WorkspaceFileSymlinkRejected(_)
        ));
    }
}

#[test]
fn workspace_file_attachment_is_revision_bound_and_durable_in_the_user_turn() {
    let service = AppService::bootstrap().expect("service should bootstrap");
    let root = tempfile::tempdir().expect("workspace tempdir");
    std::fs::write(root.path().join("context.md"), "bounded context\n")
        .expect("context should write");
    let (project_id, workspace_id) = open_project_fixture(&service, root.path());
    let session = service
        .open_session(
            TEST_CLIENT_NAME,
            TEST_OWNER_PRINCIPAL_ID,
            &crate::orchestration::OpenSessionRequest {
                title: "Attachment boundary".to_string(),
                workspace_id: workspace_id.clone(),
            },
        )
        .expect("session should open")
        .session;
    let read = service
        .read_workspace_file(
            TEST_OWNER_PRINCIPAL_ID,
            &WorkspaceFileReadParams {
                project_id,
                workspace_id,
                path: "context.md".to_string(),
                pdf_page_index: None,
            },
        )
        .expect("context should read");
    let revision = read.content.revision().to_string();
    let mut command = start_run_command(&service, "Use the attached context");
    command.attachments = vec![WorkspaceFileAttachmentRequest {
        path: "context.md".to_string(),
        expected_revision: revision.clone(),
    }];

    let started = service
        .start_run(&session.id, &command)
        .expect("validated attachment should start");
    let run = service
        .store
        .lock()
        .expect("app store should not be poisoned")
        .run(&started.body.id)
        .expect("run lookup should succeed")
        .expect("run should exist");
    assert_eq!(run.objective, "Use the attached context");
    assert!(matches!(
        &run.source,
        RunSource::User { attachments, .. }
            if attachments.len() == 1
                && attachments[0].path == "context.md"
                && attachments[0].revision == revision
                && attachments[0].byte_len == 16
    ));

    let page = service
        .agent_turns_page(
            &session.id,
            &AgentTurnsPageQuery {
                limit: 10,
                before: None,
            },
        )
        .expect("agent turns page should load");
    assert!(page.items.iter().any(|row| matches!(
        row,
        crate::AgentTurnRow::User(user)
            if user.text == "Use the attached context"
                && user.attachments.len() == 1
                && user.attachments[0].path == "context.md"
                && user.attachments[0].revision == revision
    )));
}

#[test]
fn workspace_file_attachments_reject_duplicates_stale_revisions_and_excess_count() {
    let service = AppService::bootstrap().expect("service should bootstrap");
    let root = tempfile::tempdir().expect("workspace tempdir");
    std::fs::write(root.path().join("context.txt"), "first\n").expect("context should write");
    let (project_id, workspace_id) = open_project_fixture(&service, root.path());
    let session = service
        .open_session(
            TEST_CLIENT_NAME,
            TEST_OWNER_PRINCIPAL_ID,
            &crate::orchestration::OpenSessionRequest {
                title: "Attachment validation".to_string(),
                workspace_id: workspace_id.clone(),
            },
        )
        .expect("session should open")
        .session;
    let read = service
        .read_workspace_file(
            TEST_OWNER_PRINCIPAL_ID,
            &WorkspaceFileReadParams {
                project_id,
                workspace_id,
                path: "context.txt".to_string(),
                pdf_page_index: None,
            },
        )
        .expect("context should read");
    let request = WorkspaceFileAttachmentRequest {
        path: "context.txt".to_string(),
        expected_revision: read.content.revision().to_string(),
    };

    let mut duplicate = start_run_command(&service, "Reject duplicate attachments");
    duplicate.attachments = vec![request.clone(), request.clone()];
    assert!(matches!(
        service
            .start_run(&session.id, &duplicate)
            .expect_err("duplicate attachments should fail"),
        AppServiceError::WorkspaceFileAttachmentDuplicate(path) if path == "context.txt"
    ));

    std::fs::write(root.path().join("context.txt"), "second\n").expect("context should change");
    let mut stale = start_run_command(&service, "Reject stale attachments");
    stale.attachments = vec![request.clone()];
    assert!(matches!(
        service
            .start_run(&session.id, &stale)
            .expect_err("stale attachment should fail"),
        AppServiceError::WorkspaceFileStale(path) if path == "context.txt"
    ));

    let mut excess = start_run_command(&service, "Reject excess attachments");
    excess.attachments = vec![request; WORKSPACE_FILE_ATTACHMENT_MAX_COUNT + 1];
    assert!(matches!(
        service
            .start_run(&session.id, &excess)
            .expect_err("excess attachment count should fail"),
        AppServiceError::WorkspaceFileAttachmentLimitExceeded { max }
            if max == WORKSPACE_FILE_ATTACHMENT_MAX_COUNT
    ));
    assert!(
        service
            .list_runs(&session.id)
            .expect("runs should list")
            .is_empty()
    );
}

#[test]
fn workspace_image_attachment_preflight_rejects_unsafe_input_before_run_persistence() {
    let service = AppService::bootstrap().expect("service should bootstrap");
    let root = tempfile::tempdir().expect("workspace tempdir");
    std::fs::write(root.path().join("unsafe.png"), b"not an image")
        .expect("unsafe fixture should write");
    let (project_id, workspace_id) = open_project_fixture(&service, root.path());
    let session = service
        .open_session(
            TEST_CLIENT_NAME,
            TEST_OWNER_PRINCIPAL_ID,
            &crate::orchestration::OpenSessionRequest {
                title: "Unsafe image attachment".to_string(),
                workspace_id: workspace_id.clone(),
            },
        )
        .expect("session should open")
        .session;
    let mut command = start_run_command(&service, "Reject unsafe image");
    command.attachments = vec![WorkspaceFileAttachmentRequest {
        path: "unsafe.png".to_string(),
        expected_revision: "sha256:stale".to_string(),
    }];

    let error = service
        .start_run(&session.id, &command)
        .expect_err("unsafe image must fail before a run or user turn is persisted");
    assert!(matches!(
        error,
        AppServiceError::WorkspaceFileUnsupportedKind(path) if path == "unsafe.png"
    ));
    assert!(
        service
            .list_runs(&session.id)
            .expect("runs should list")
            .is_empty(),
        "attachment preflight must not persist a run"
    );
    assert!(
        service
            .agent_turns_page(
                &session.id,
                &AgentTurnsPageQuery {
                    limit: 10,
                    before: None,
                },
            )
            .expect("agent turns should list")
            .items
            .is_empty(),
        "attachment preflight must not persist a user turn"
    );
    let _ = project_id;
}

fn minimal_pdf() -> Vec<u8> {
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>",
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] /Resources <<>> /Contents 4 0 R >>",
        "<< /Length 0 >>\nstream\n\nendstream",
    ];
    let mut pdf = String::from("%PDF-1.4\n");
    let mut offsets = Vec::with_capacity(objects.len());
    for (index, object) in objects.iter().enumerate() {
        offsets.push(pdf.len());
        pdf.push_str(&format!("{} 0 obj\n{}\nendobj\n", index + 1, object));
    }
    let xref = pdf.len();
    pdf.push_str("xref\n0 5\n0000000000 65535 f \n");
    for offset in offsets {
        pdf.push_str(&format!("{offset:010} 00000 n \n"));
    }
    pdf.push_str(&format!(
        "trailer\n<< /Size 5 /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n"
    ));
    pdf.into_bytes()
}
