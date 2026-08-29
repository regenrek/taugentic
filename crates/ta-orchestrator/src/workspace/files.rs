use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Component, Path, PathBuf},
    sync::Arc,
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use hayro::{RenderSettings, hayro_interpret::InterpreterSettings, hayro_syntax::Pdf, render};
use ignore::WalkBuilder;
use sha2::{Digest, Sha256};
use ta_protocol::wire::{
    BoundedFileContent, WORKSPACE_BINARY_MAX_BYTES, WORKSPACE_FILE_TREE_MAX_ENTRIES,
    WORKSPACE_PDF_MAX_BYTES, WORKSPACE_TEXT_MAX_BYTES, WorkspaceFileAttachment,
    WorkspaceFileAttachmentRequest, WorkspaceFileEntry, WorkspaceFileKind, WorkspaceFileTreeResult,
};
use uuid::Uuid;

const PDF_PREVIEW_MAX_WIDTH: f32 = 1_280.0;
const PDF_PREVIEW_MAX_HEIGHT: f32 = 1_800.0;
const PDF_PREVIEW_MAX_SCALE: f32 = 2.0;

use crate::orchestration::AppServiceError;

pub(crate) fn workspace_file_tree(root: &Path) -> Result<WorkspaceFileTreeResult, AppServiceError> {
    let root = canonical_root(root)?;
    let filter_root = root.clone();
    let mut builder = WalkBuilder::new(&root);
    builder
        .hidden(false)
        .git_ignore(true)
        .git_global(false)
        .git_exclude(true)
        .parents(true)
        .follow_links(false)
        .filter_entry(move |entry| !excluded_directory(entry.path(), &filter_root));

    let mut entries = Vec::new();
    let mut truncated = false;
    for result in builder.build() {
        let entry = result.map_err(|error| AppServiceError::WorkspaceFileIo {
            path: root.display().to_string(),
            action: "walk".to_string(),
            reason: error.to_string(),
        })?;
        if entry.path() == root {
            continue;
        }
        if entries.len() == WORKSPACE_FILE_TREE_MAX_ENTRIES {
            truncated = true;
            break;
        }

        let path = relative_display(&root, entry.path());
        if entry.path_is_symlink() {
            entries.push(WorkspaceFileEntry {
                name: entry.file_name().to_string_lossy().into_owned(),
                kind: WorkspaceFileKind::Binary,
                is_symlink: true,
                byte_len: 0,
                path,
            });
            continue;
        }

        let canonical =
            entry
                .path()
                .canonicalize()
                .map_err(|error| AppServiceError::WorkspaceFileIo {
                    path: entry.path().display().to_string(),
                    action: "canonicalize".to_string(),
                    reason: error.to_string(),
                })?;
        ensure_contained(&root, &canonical, entry.path())?;
        let metadata =
            fs::metadata(&canonical).map_err(|error| AppServiceError::WorkspaceFileIo {
                path: relative_display(&root, entry.path()),
                action: "metadata".to_string(),
                reason: error.to_string(),
            })?;
        entries.push(WorkspaceFileEntry {
            name: entry.file_name().to_string_lossy().into_owned(),
            kind: if metadata.is_dir() {
                WorkspaceFileKind::Directory
            } else {
                classify_path(entry.path())
            },
            is_symlink: entry.path_is_symlink(),
            byte_len: if metadata.is_file() {
                metadata.len()
            } else {
                0
            },
            path,
        });
    }
    entries.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(WorkspaceFileTreeResult { entries, truncated })
}

pub(crate) fn read_workspace_file(
    root: &Path,
    relative_path: &str,
    pdf_page_index: Option<u32>,
) -> Result<(String, PathBuf, BoundedFileContent), AppServiceError> {
    let root = canonical_root(root)?;
    let (path, relative_path) = contained_existing_file(&root, relative_path)?;
    let content = read_bounded_file(&path, &relative_path, pdf_page_index)?;
    Ok((relative_path, path, content))
}

pub(crate) fn validate_workspace_file_attachment(
    root: &Path,
    request: &WorkspaceFileAttachmentRequest,
) -> Result<WorkspaceFileAttachment, AppServiceError> {
    let root = canonical_root(root)?;
    let (path, relative_path) = contained_existing_file(&root, &request.path)?;
    let kind = classify_path(&path);
    let bytes = read_bytes(
        &path,
        &relative_path,
        max_bytes_for_kind(kind, &relative_path)?,
    )?;
    validate_file_kind(&path, &relative_path, kind, &bytes)?;
    let revision = revision(&bytes);
    if revision != request.expected_revision {
        return Err(AppServiceError::WorkspaceFileStale(relative_path));
    }
    Ok(WorkspaceFileAttachment {
        path: relative_path,
        revision,
        kind,
        byte_len: bytes.len() as u64,
    })
}

pub(crate) fn read_artifact_file(
    root: &Path,
    storage_path: &str,
    pdf_page_index: Option<u32>,
) -> Result<BoundedFileContent, AppServiceError> {
    let root = canonical_root(root)?;
    let storage_path_value = storage_path;
    let storage_path = Path::new(storage_path_value);
    let relative = if storage_path.is_absolute() {
        storage_path
            .strip_prefix(&root)
            .map(Path::to_path_buf)
            .map_err(|_| AppServiceError::ArtifactContentUnavailable {
                reason: "artifact path is outside the artifact root".to_string(),
            })?
    } else {
        validated_relative_path(storage_path.to_str().ok_or_else(|| {
            AppServiceError::ArtifactContentUnavailable {
                reason: "artifact path is not valid UTF-8".to_string(),
            }
        })?)
        .map_err(|error| AppServiceError::ArtifactContentUnavailable {
            reason: error.to_string(),
        })?
    };
    if relative
        .components()
        .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(AppServiceError::ArtifactContentUnavailable {
            reason: "artifact path contains invalid components".to_string(),
        });
    }
    ensure_no_symlink_components(&root, &relative).map_err(|error| {
        AppServiceError::ArtifactContentUnavailable {
            reason: error.to_string(),
        }
    })?;
    let candidate = root.join(&relative);
    let canonical =
        candidate
            .canonicalize()
            .map_err(|error| AppServiceError::ArtifactContentUnavailable {
                reason: error.to_string(),
            })?;
    if !canonical.starts_with(&root) || !canonical.is_file() {
        return Err(AppServiceError::ArtifactContentUnavailable {
            reason: "artifact path is not a contained regular file".to_string(),
        });
    }
    read_bounded_file(&canonical, storage_path_value, pdf_page_index).map_err(|error| {
        AppServiceError::ArtifactContentUnavailable {
            reason: error.to_string(),
        }
    })
}

pub(crate) fn write_workspace_text_file(
    root: &Path,
    relative_path: &str,
    expected_revision: &str,
    text: &str,
) -> Result<(String, String, u64), AppServiceError> {
    let root = canonical_root(root)?;
    let (path, relative_path) = contained_existing_file(&root, relative_path)?;
    let metadata = fs::metadata(&path).map_err(|error| AppServiceError::WorkspaceFileIo {
        path: relative_path.clone(),
        action: "metadata".to_string(),
        reason: error.to_string(),
    })?;
    let current = read_bytes(&path, &relative_path, WORKSPACE_TEXT_MAX_BYTES)?;
    if std::str::from_utf8(&current).is_err() {
        return Err(AppServiceError::WorkspaceFileUnsupportedKind(relative_path));
    }
    if revision(&current) != expected_revision {
        return Err(AppServiceError::WorkspaceFileStale(relative_path));
    }
    let bytes = text.as_bytes();
    if bytes.len() as u64 > WORKSPACE_TEXT_MAX_BYTES {
        return Err(AppServiceError::WorkspaceFileTooLarge {
            path: relative_path,
            max_bytes: WORKSPACE_TEXT_MAX_BYTES,
        });
    }
    let parent = path
        .parent()
        .ok_or_else(|| AppServiceError::WorkspaceFileInvalidPath {
            path: relative_path.clone(),
        })?;
    let parent_canonical =
        parent
            .canonicalize()
            .map_err(|error| AppServiceError::WorkspaceFileIo {
                path: relative_path.clone(),
                action: "canonicalize parent".to_string(),
                reason: error.to_string(),
            })?;
    ensure_contained(&root, &parent_canonical, parent)?;
    let temporary = parent.join(format!(".taugentic-save-{}", Uuid::new_v4().simple()));
    let write_result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.set_permissions(metadata.permissions())?;
        file.write_all(bytes)?;
        file.sync_all()?;
        fs::rename(&temporary, &path)?;
        Ok::<(), std::io::Error>(())
    })();
    if let Err(error) = write_result {
        let _ = fs::remove_file(&temporary);
        return Err(AppServiceError::WorkspaceFileIo {
            path: relative_path,
            action: "save".to_string(),
            reason: error.to_string(),
        });
    }
    Ok((relative_path, revision(bytes), bytes.len() as u64))
}

fn read_bounded_file(
    path: &Path,
    display_path: &str,
    pdf_page_index: Option<u32>,
) -> Result<BoundedFileContent, AppServiceError> {
    let expected_kind = classify_path(path);
    let max_bytes = max_bytes_for_kind(expected_kind, display_path)?;
    let bytes = read_bytes(path, display_path, max_bytes)?;
    let media_type = validate_file_kind(path, display_path, expected_kind, &bytes)?;
    let revision = revision(&bytes);
    let byte_len = bytes.len() as u64;

    if expected_kind == WorkspaceFileKind::Pdf {
        return render_pdf_page(bytes, display_path, pdf_page_index, revision, byte_len);
    }
    if expected_kind == WorkspaceFileKind::Image {
        let media_type = media_type.expect("validated image should have a media type");
        return Ok(BoundedFileContent::Image {
            data_uri: format!("data:{media_type};base64,{}", BASE64.encode(bytes)),
            media_type: media_type.to_string(),
            revision,
            byte_len,
        });
    }
    if let Ok(text) = String::from_utf8(bytes.clone()) {
        return Ok(BoundedFileContent::Text {
            text,
            revision,
            language: language_for_path(path).map(str::to_string),
            byte_len,
        });
    }
    Ok(BoundedFileContent::Binary {
        data_base64: BASE64.encode(bytes),
        media_type: None,
        revision,
        byte_len,
    })
}

fn max_bytes_for_kind(kind: WorkspaceFileKind, display_path: &str) -> Result<u64, AppServiceError> {
    match kind {
        WorkspaceFileKind::Text => Ok(WORKSPACE_TEXT_MAX_BYTES),
        WorkspaceFileKind::Pdf => Ok(WORKSPACE_PDF_MAX_BYTES),
        WorkspaceFileKind::Image | WorkspaceFileKind::Binary => Ok(WORKSPACE_BINARY_MAX_BYTES),
        WorkspaceFileKind::Directory => Err(AppServiceError::WorkspaceFileNotRegular(
            display_path.to_string(),
        )),
    }
}

fn validate_file_kind(
    path: &Path,
    display_path: &str,
    kind: WorkspaceFileKind,
    bytes: &[u8],
) -> Result<Option<&'static str>, AppServiceError> {
    match kind {
        WorkspaceFileKind::Text if std::str::from_utf8(bytes).is_err() => Err(
            AppServiceError::WorkspaceFileUnsupportedKind(display_path.to_string()),
        ),
        WorkspaceFileKind::Image => image_media_type(path, bytes)
            .map(Some)
            .ok_or_else(|| AppServiceError::WorkspaceFileUnsupportedKind(display_path.to_string())),
        WorkspaceFileKind::Pdf if !bytes.starts_with(b"%PDF-") => Err(
            AppServiceError::WorkspaceFileUnsupportedKind(display_path.to_string()),
        ),
        WorkspaceFileKind::Directory => Err(AppServiceError::WorkspaceFileNotRegular(
            display_path.to_string(),
        )),
        WorkspaceFileKind::Text | WorkspaceFileKind::Pdf | WorkspaceFileKind::Binary => Ok(None),
    }
}

fn render_pdf_page(
    bytes: Vec<u8>,
    display_path: &str,
    requested_page_index: Option<u32>,
    revision: String,
    byte_len: u64,
) -> Result<BoundedFileContent, AppServiceError> {
    let pdf = Pdf::new(Arc::new(bytes))
        .map_err(|_| AppServiceError::WorkspaceFileInvalidPdf(display_path.to_string()))?;
    let page_count = u32::try_from(pdf.pages().len())
        .map_err(|_| AppServiceError::WorkspaceFileInvalidPdf(display_path.to_string()))?;
    let page_index = requested_page_index.unwrap_or(0);
    let page = pdf.pages().get(page_index as usize).ok_or_else(|| {
        AppServiceError::WorkspaceFilePdfPageOutOfRange {
            path: display_path.to_string(),
            page_index,
            page_count,
        }
    })?;
    let (width, height) = page.render_dimensions();
    if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
        return Err(AppServiceError::WorkspaceFileInvalidPdf(
            display_path.to_string(),
        ));
    }
    let scale = (PDF_PREVIEW_MAX_WIDTH / width)
        .min(PDF_PREVIEW_MAX_HEIGHT / height)
        .min(PDF_PREVIEW_MAX_SCALE)
        .max(f32::EPSILON);
    let target_width = (width * scale).round().clamp(1.0, PDF_PREVIEW_MAX_WIDTH) as u16;
    let target_height = (height * scale).round().clamp(1.0, PDF_PREVIEW_MAX_HEIGHT) as u16;
    let pixmap = render(
        page,
        &InterpreterSettings::default(),
        &RenderSettings {
            x_scale: scale,
            y_scale: scale,
            width: Some(target_width),
            height: Some(target_height),
            bg_color: hayro::vello_cpu::color::palette::css::WHITE,
        },
    );
    let png = pixmap
        .into_png()
        .map_err(|_| AppServiceError::WorkspaceFileInvalidPdf(display_path.to_string()))?;
    if png.len() as u64 > WORKSPACE_BINARY_MAX_BYTES {
        return Err(AppServiceError::WorkspaceFileTooLarge {
            path: display_path.to_string(),
            max_bytes: WORKSPACE_BINARY_MAX_BYTES,
        });
    }
    Ok(BoundedFileContent::Pdf {
        preview_data_uri: format!("data:image/png;base64,{}", BASE64.encode(png)),
        page_index,
        page_count,
        revision,
        byte_len,
    })
}

fn read_bytes(path: &Path, display_path: &str, max_bytes: u64) -> Result<Vec<u8>, AppServiceError> {
    let metadata = fs::metadata(path).map_err(|error| AppServiceError::WorkspaceFileIo {
        path: display_path.to_string(),
        action: "metadata".to_string(),
        reason: error.to_string(),
    })?;
    if !metadata.is_file() {
        return Err(AppServiceError::WorkspaceFileNotRegular(
            display_path.to_string(),
        ));
    }
    if metadata.len() > max_bytes {
        return Err(AppServiceError::WorkspaceFileTooLarge {
            path: display_path.to_string(),
            max_bytes,
        });
    }
    let bytes = fs::read(path).map_err(|error| AppServiceError::WorkspaceFileIo {
        path: display_path.to_string(),
        action: "read".to_string(),
        reason: error.to_string(),
    })?;
    if bytes.len() as u64 > max_bytes {
        return Err(AppServiceError::WorkspaceFileTooLarge {
            path: display_path.to_string(),
            max_bytes,
        });
    }
    Ok(bytes)
}

fn canonical_root(root: &Path) -> Result<PathBuf, AppServiceError> {
    root.canonicalize()
        .map_err(|error| AppServiceError::WorkspaceFileIo {
            path: root.display().to_string(),
            action: "canonicalize workspace".to_string(),
            reason: error.to_string(),
        })
}

fn contained_existing_file(
    root: &Path,
    relative_path: &str,
) -> Result<(PathBuf, String), AppServiceError> {
    let relative = validated_relative_path(relative_path)?;
    ensure_no_symlink_components(root, &relative)?;
    let display = relative_display(root, &root.join(&relative));
    let candidate = root.join(relative);
    let canonical = candidate
        .canonicalize()
        .map_err(|_| AppServiceError::WorkspaceFileNotFound(display.clone()))?;
    ensure_contained(root, &canonical, &candidate)?;
    if !canonical.is_file() {
        return Err(AppServiceError::WorkspaceFileNotRegular(display));
    }
    Ok((canonical, display))
}

fn ensure_no_symlink_components(root: &Path, relative: &Path) -> Result<(), AppServiceError> {
    let mut current = root.to_path_buf();
    for component in relative.components() {
        let Component::Normal(component) = component else {
            return Err(AppServiceError::WorkspaceFileInvalidPath {
                path: relative.display().to_string(),
            });
        };
        current.push(component);
        let metadata = fs::symlink_metadata(&current)
            .map_err(|_| AppServiceError::WorkspaceFileNotFound(relative.display().to_string()))?;
        if metadata.file_type().is_symlink() {
            return Err(AppServiceError::WorkspaceFileSymlinkRejected(
                relative.display().to_string(),
            ));
        }
    }
    Ok(())
}

fn validated_relative_path(value: &str) -> Result<PathBuf, AppServiceError> {
    let path = Path::new(value);
    if value.trim().is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(AppServiceError::WorkspaceFileInvalidPath {
            path: value.to_string(),
        });
    }
    Ok(path.to_path_buf())
}

fn ensure_contained(
    root: &Path,
    canonical: &Path,
    requested: &Path,
) -> Result<(), AppServiceError> {
    if canonical.starts_with(root) {
        return Ok(());
    }
    Err(AppServiceError::WorkspaceSymlinkEscape(
        requested.display().to_string(),
    ))
}

fn excluded_directory(path: &Path, root: &Path) -> bool {
    if path == root {
        return false;
    }
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| matches!(name, ".git" | "node_modules" | "target"))
}

fn relative_display(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value.to_string_lossy()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn classify_path(path: &Path) -> WorkspaceFileKind {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if matches!(
        extension.as_str(),
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" | "bmp" | "tif" | "tiff" | "ico"
    ) {
        return WorkspaceFileKind::Image;
    }
    if extension == "pdf" {
        return WorkspaceFileKind::Pdf;
    }
    if language_for_path(path).is_some()
        || matches!(
            extension.as_str(),
            "txt"
                | "md"
                | "mdx"
                | "json"
                | "jsonl"
                | "yaml"
                | "yml"
                | "toml"
                | "xml"
                | "html"
                | "css"
                | "csv"
                | "log"
                | "diff"
                | "patch"
        )
    {
        return WorkspaceFileKind::Text;
    }
    WorkspaceFileKind::Binary
}

fn language_for_path(path: &Path) -> Option<&'static str> {
    match path.extension()?.to_str()?.to_ascii_lowercase().as_str() {
        "rs" => Some("rust"),
        "ts" | "tsx" => Some("typescript"),
        "js" | "jsx" | "mjs" | "cjs" => Some("javascript"),
        "py" => Some("python"),
        "go" => Some("go"),
        "java" => Some("java"),
        "kt" | "kts" => Some("kotlin"),
        "swift" => Some("swift"),
        "c" | "h" => Some("c"),
        "cc" | "cpp" | "cxx" | "hpp" => Some("cpp"),
        "sh" | "bash" | "zsh" => Some("bash"),
        "json" | "jsonl" => Some("json"),
        "yaml" | "yml" => Some("yaml"),
        "toml" => Some("toml"),
        "html" => Some("html"),
        "css" => Some("css"),
        "md" | "mdx" => Some("markdown"),
        "diff" | "patch" => Some("diff"),
        _ => None,
    }
}

fn image_media_type(path: &Path, bytes: &[u8]) -> Option<&'static str> {
    let extension = path.extension()?.to_str()?.to_ascii_lowercase();
    match extension.as_str() {
        "png" if bytes.starts_with(b"\x89PNG\r\n\x1a\n") => Some("image/png"),
        "jpg" | "jpeg" if bytes.starts_with(b"\xff\xd8\xff") => Some("image/jpeg"),
        "gif" if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") => Some("image/gif"),
        "webp" if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") => {
            Some("image/webp")
        }
        _ => None,
    }
}

fn revision(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
