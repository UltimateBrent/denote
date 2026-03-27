mod fixtures;

use denote::config::{AttachmentMode, ExportConfig, FilenameStrategy};
use denote::db::reader::{BearReader, NoteSource};
use denote::export::diff::{self, Change};
use denote::export::filemap::Manifest;

fn default_export_config() -> ExportConfig {
    ExportConfig {
        frontmatter: true,
        attachment_mode: AttachmentMode::Placeholder,
        filename_strategy: FilenameStrategy::TitleUuid,
    }
}

#[test]
fn test_diff_all_new() {
    let dir = std::env::temp_dir().join("denote-test-diff-new");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let db_path = fixtures::create_test_bear_db(&dir);
    let reader = BearReader::new(&db_path).unwrap();
    let notes = reader.fetch_notes(false, false).unwrap();

    let repo_dir = dir.join("repo");
    std::fs::create_dir_all(&repo_dir).unwrap();

    let manifest = Manifest::default();
    let export_config = default_export_config();
    let changes = diff::compute_diff(&notes, &manifest, &repo_dir, &[], &export_config);

    let added = changes
        .iter()
        .filter(|c| matches!(c, Change::Added(_)))
        .count();
    assert_eq!(added, 3);

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn test_diff_no_changes_after_apply() {
    let dir = std::env::temp_dir().join("denote-test-diff-idempotent");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let db_path = fixtures::create_test_bear_db(&dir);
    let reader = BearReader::new(&db_path).unwrap();
    let notes = reader.fetch_notes(false, false).unwrap();

    let repo_dir = dir.join("repo");
    std::fs::create_dir_all(&repo_dir).unwrap();

    let mut manifest = Manifest::default();
    let export_config = default_export_config();

    // First pass: apply all changes
    let changes = diff::compute_diff(&notes, &manifest, &repo_dir, &[], &export_config);
    let count = diff::apply_changes(&changes, &mut manifest, &repo_dir, &export_config).unwrap();
    assert_eq!(count, 3);

    // Second pass: no changes
    let changes = diff::compute_diff(&notes, &manifest, &repo_dir, &[], &export_config);
    assert!(changes.is_empty(), "Expected no changes after applying all");

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn test_diff_with_tag_exclusion() {
    let dir = std::env::temp_dir().join("denote-test-diff-exclude");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let db_path = fixtures::create_test_bear_db(&dir);
    let reader = BearReader::new(&db_path).unwrap();
    let notes = reader.fetch_notes(false, false).unwrap();

    let repo_dir = dir.join("repo");
    std::fs::create_dir_all(&repo_dir).unwrap();

    let manifest = Manifest::default();
    let export_config = default_export_config();
    let exclude = vec!["private".to_string()];

    let changes = diff::compute_diff(&notes, &manifest, &repo_dir, &exclude, &export_config);
    let added = changes
        .iter()
        .filter(|c| matches!(c, Change::Added(_)))
        .count();
    // "Private Stuff" has only the "private" tag, so it should be excluded
    assert_eq!(added, 2);

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn test_diff_detects_modification() {
    let dir = std::env::temp_dir().join("denote-test-diff-modify");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let db_path = fixtures::create_test_bear_db(&dir);
    let reader = BearReader::new(&db_path).unwrap();
    let notes = reader.fetch_notes(false, false).unwrap();

    let repo_dir = dir.join("repo");
    std::fs::create_dir_all(&repo_dir).unwrap();

    let mut manifest = Manifest::default();
    let export_config = default_export_config();

    // Apply initial changes
    let changes = diff::compute_diff(&notes, &manifest, &repo_dir, &[], &export_config);
    diff::apply_changes(&changes, &mut manifest, &repo_dir, &export_config).unwrap();

    // Tamper with a file to simulate modification
    let project_filename = manifest.filename_for("NOTE-UUID-AAAA").unwrap();
    std::fs::write(repo_dir.join(project_filename), "stale content").unwrap();

    // Detect modification
    let changes = diff::compute_diff(&notes, &manifest, &repo_dir, &[], &export_config);
    assert_eq!(changes.len(), 1);
    assert!(matches!(&changes[0], Change::Modified(n) if n.id == "NOTE-UUID-AAAA"));

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn test_diff_detects_deletion() {
    let dir = std::env::temp_dir().join("denote-test-diff-delete");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let db_path = fixtures::create_test_bear_db(&dir);
    let reader = BearReader::new(&db_path).unwrap();
    let notes = reader.fetch_notes(false, false).unwrap();

    let repo_dir = dir.join("repo");
    std::fs::create_dir_all(&repo_dir).unwrap();

    let mut manifest = Manifest::default();
    let export_config = default_export_config();

    // Apply initial changes
    let changes = diff::compute_diff(&notes, &manifest, &repo_dir, &[], &export_config);
    diff::apply_changes(&changes, &mut manifest, &repo_dir, &export_config).unwrap();

    // Simulate a note being trashed: remove it from the notes list
    let notes_without_project: Vec<_> = notes
        .into_iter()
        .filter(|n| n.id != "NOTE-UUID-AAAA")
        .collect();

    let changes = diff::compute_diff(
        &notes_without_project,
        &manifest,
        &repo_dir,
        &[],
        &export_config,
    );

    let deleted = changes
        .iter()
        .filter(|c| matches!(c, Change::Deleted { id, .. } if id == "NOTE-UUID-AAAA"))
        .count();
    assert_eq!(deleted, 1);

    // Apply deletion
    let count =
        diff::apply_changes(&changes, &mut manifest, &repo_dir, &export_config).unwrap();
    assert_eq!(count, 1);
    assert!(manifest.filename_for("NOTE-UUID-AAAA").is_none());

    std::fs::remove_dir_all(&dir).ok();
}
