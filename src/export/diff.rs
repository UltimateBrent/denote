use std::collections::HashSet;
use std::path::{Path, PathBuf};

use tracing::{debug, warn};

use crate::config::{AttachmentMode, ExportConfig};
use crate::db::models::BearNote;
use crate::export::filemap::Manifest;
use crate::export::markdown;

#[derive(Debug)]
pub enum Change {
    Added(BearNote),
    Modified(BearNote),
    Deleted { id: String, filename: String },
}

/// Compute the set of changes needed to bring the repo in sync with the current notes.
pub fn compute_diff(
    notes: &[BearNote],
    manifest: &Manifest,
    repo_path: &Path,
    exclude_tags: &[String],
    export_config: &ExportConfig,
) -> Vec<Change> {
    let notes = filter_excluded(notes, exclude_tags);

    let note_ids: HashSet<&str> = notes.iter().map(|n| n.id.as_str()).collect();
    let mut changes = Vec::new();

    for &note in &notes {
        match manifest.filename_for(&note.id) {
            None => {
                changes.push(Change::Added(note.clone()));
            }
            Some(filename) => {
                let file_path = repo_path.join(filename);
                if is_modified(note, &file_path, export_config) {
                    changes.push(Change::Modified(note.clone()));
                }
            }
        }
    }

    for (uuid, filename) in &manifest.entries {
        if !note_ids.contains(uuid.as_str()) {
            changes.push(Change::Deleted {
                id: uuid.clone(),
                filename: filename.clone(),
            });
        }
    }

    changes
}

/// Notes where ALL tags are in the exclude list are filtered out.
/// Notes with at least one non-excluded tag pass through.
/// Notes with no tags also pass through.
fn filter_excluded<'a>(notes: &'a [BearNote], exclude_tags: &[String]) -> Vec<&'a BearNote> {
    if exclude_tags.is_empty() {
        return notes.iter().collect();
    }

    let exclude_set: HashSet<&str> = exclude_tags.iter().map(|s| s.as_str()).collect();

    notes
        .iter()
        .filter(|note| {
            if note.tags.is_empty() {
                return true;
            }
            note.tags.iter().any(|t| !exclude_set.contains(t.as_str()))
        })
        .collect()
}

/// Check if a note's rendered content differs from what's on disk.
fn is_modified(note: &BearNote, file_path: &Path, export_config: &ExportConfig) -> bool {
    let assets_subdir = assets_subdir_for(note);
    let rendered = markdown::render(
        note,
        export_config.frontmatter,
        &export_config.attachment_mode,
        assets_subdir.as_deref(),
    );

    match std::fs::read_to_string(file_path) {
        Ok(existing) => existing != rendered,
        Err(_) => true, // file missing = modified
    }
}

fn assets_subdir_for(note: &BearNote) -> Option<String> {
    if note.attachments.is_empty() {
        None
    } else {
        let short_id = &note.id[..note.id.len().min(8)];
        Some(format!("_assets/{short_id}"))
    }
}

/// Apply a set of changes to the repo: write files, remove deleted ones, update manifest.
pub fn apply_changes(
    changes: &[Change],
    manifest: &mut Manifest,
    repo_path: &Path,
    export_config: &ExportConfig,
    bear_db_path: Option<&Path>,
) -> crate::errors::Result<usize> {
    let mut count = 0;

    for change in changes {
        match change {
            Change::Added(note) | Change::Modified(note) => {
                let filename = match change {
                    Change::Added(_) => {
                        manifest.generate_filename(note, &export_config.filename_strategy)
                    }
                    Change::Modified(_) => manifest
                        .filename_for(&note.id)
                        .cloned()
                        .unwrap_or_else(|| {
                            manifest.generate_filename(note, &export_config.filename_strategy)
                        }),
                    _ => unreachable!(),
                };

                let assets_subdir = assets_subdir_for(note);
                let content = markdown::render(
                    note,
                    export_config.frontmatter,
                    &export_config.attachment_mode,
                    assets_subdir.as_deref(),
                );
                std::fs::write(repo_path.join(&filename), content)?;

                if matches!(export_config.attachment_mode, AttachmentMode::Copy) {
                    if let Some(db_path) = bear_db_path {
                        copy_attachments(note, repo_path, db_path)?;
                    }
                }

                manifest.set(note.id.clone(), filename);
                count += 1;
            }
            Change::Deleted { id, filename } => {
                let file_path = repo_path.join(filename);
                if file_path.exists() {
                    std::fs::remove_file(&file_path)?;
                }
                if matches!(export_config.attachment_mode, AttachmentMode::Copy) {
                    let short_id = &id[..id.len().min(8)];
                    let assets_dir = repo_path.join("_assets").join(short_id);
                    if assets_dir.is_dir() {
                        std::fs::remove_dir_all(&assets_dir)?;
                        debug!(dir = %assets_dir.display(), "Removed assets directory");
                    }
                }
                manifest.remove(id);
                count += 1;
            }
        }
    }

    manifest.save(repo_path)?;
    Ok(count)
}

fn copy_attachments(
    note: &BearNote,
    repo_path: &Path,
    bear_db_path: &Path,
) -> crate::errors::Result<()> {
    if note.attachments.is_empty() {
        return Ok(());
    }

    let short_id = &note.id[..note.id.len().min(8)];
    let assets_dir: PathBuf = repo_path.join("_assets").join(short_id);
    std::fs::create_dir_all(&assets_dir)?;

    for attachment in &note.attachments {
        let src = attachment.source_path(bear_db_path);
        let dst = assets_dir.join(&attachment.filename);

        if src.exists() {
            std::fs::copy(&src, &dst)?;
            debug!(
                src = %src.display(),
                dst = %dst.display(),
                "Copied attachment"
            );
        } else {
            warn!(
                path = %src.display(),
                note_id = %note.id,
                "Attachment file not found, skipping"
            );
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AttachmentMode, FilenameStrategy};
    use time::OffsetDateTime;

    fn note(id: &str, title: &str, tags: Vec<&str>) -> BearNote {
        BearNote {
            id: id.into(),
            title: title.into(),
            text: format!("# {title}\n\nContent for {id}\n"),
            tags: tags.into_iter().map(String::from).collect(),
            created: OffsetDateTime::UNIX_EPOCH,
            modified: OffsetDateTime::UNIX_EPOCH,
            is_trashed: false,
            is_archived: false,
            is_pinned: false,
            attachments: vec![],
        }
    }

    #[test]
    fn test_all_new_notes() {
        let notes = vec![note("A", "Alpha", vec!["work"]), note("B", "Beta", vec!["home"])];
        let manifest = Manifest::default();
        let export_config = ExportConfig::default();
        let changes = compute_diff(&notes, &manifest, Path::new("/tmp"), &[], &export_config);

        let added_count = changes
            .iter()
            .filter(|c| matches!(c, Change::Added(_)))
            .count();
        assert_eq!(added_count, 2);
    }

    #[test]
    fn test_deleted_note() {
        let notes: Vec<BearNote> = vec![];
        let mut manifest = Manifest::default();
        manifest.set("OLD-ID".into(), "old-note.md".into());
        let export_config = ExportConfig::default();

        let changes = compute_diff(&notes, &manifest, Path::new("/tmp"), &[], &export_config);
        assert_eq!(changes.len(), 1);
        assert!(matches!(&changes[0], Change::Deleted { id, .. } if id == "OLD-ID"));
    }

    #[test]
    fn test_exclude_tags_all_excluded() {
        let notes = vec![note("A", "Private", vec!["private", "scratch"])];
        let manifest = Manifest::default();
        let exclude = vec!["private".to_string(), "scratch".to_string()];
        let export_config = ExportConfig::default();

        let changes = compute_diff(&notes, &manifest, Path::new("/tmp"), &exclude, &export_config);
        assert!(changes.is_empty());
    }

    #[test]
    fn test_exclude_tags_partial() {
        let notes = vec![note("A", "Semi-private", vec!["private", "work"])];
        let manifest = Manifest::default();
        let exclude = vec!["private".to_string()];
        let export_config = ExportConfig::default();

        let changes = compute_diff(&notes, &manifest, Path::new("/tmp"), &exclude, &export_config);
        assert_eq!(changes.len(), 1);
        assert!(matches!(&changes[0], Change::Added(_)));
    }

    #[test]
    fn test_apply_changes_roundtrip() {
        let dir = std::env::temp_dir().join("denote-test-diff");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let notes = vec![note("A", "Alpha", vec!["work"])];
        let mut manifest = Manifest::default();
        let export_config = ExportConfig {
            frontmatter: true,
            attachment_mode: AttachmentMode::Placeholder,
            filename_strategy: FilenameStrategy::TitleUuid,
        };

        let changes = compute_diff(&notes, &manifest, &dir, &[], &export_config);
        let count = apply_changes(&changes, &mut manifest, &dir, &export_config, None).unwrap();
        assert_eq!(count, 1);
        assert!(manifest.filename_for("A").is_some());

        // Second sync — no changes
        let changes = compute_diff(&notes, &manifest, &dir, &[], &export_config);
        assert!(changes.is_empty());

        std::fs::remove_dir_all(&dir).ok();
    }
}
