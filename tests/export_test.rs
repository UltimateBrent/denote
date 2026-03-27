mod fixtures;

use denote::config::{AttachmentMode, ExportConfig, FilenameStrategy};
use denote::db::reader::{BearReader, NoteSource};
use denote::export::filemap::Manifest;
use denote::export::markdown;

#[test]
fn test_read_notes_from_fixture_db() {
    let dir = std::env::temp_dir().join("denote-test-export-read");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let db_path = fixtures::create_test_bear_db(&dir);
    let reader = BearReader::new(&db_path).unwrap();

    // Default: exclude trashed and archived
    let notes = reader.fetch_notes(false, false).unwrap();
    assert_eq!(notes.len(), 3, "Should get 3 non-trashed notes");

    let project = notes.iter().find(|n| n.id == "NOTE-UUID-AAAA").unwrap();
    assert_eq!(project.title, "Project Notes");
    assert!(project.is_pinned);
    assert!(project.tags.contains(&"work".to_string()));

    let standup = notes.iter().find(|n| n.id == "NOTE-UUID-BBBB").unwrap();
    assert_eq!(standup.tags.len(), 2);
    assert!(standup.tags.contains(&"work".to_string()));
    assert!(standup.tags.contains(&"meeting".to_string()));

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn test_read_notes_include_trashed() {
    let dir = std::env::temp_dir().join("denote-test-export-trashed");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let db_path = fixtures::create_test_bear_db(&dir);
    let reader = BearReader::new(&db_path).unwrap();

    let notes = reader.fetch_notes(true, false).unwrap();
    assert_eq!(notes.len(), 4, "Should get all 4 notes including trashed");

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn test_render_exported_note() {
    let dir = std::env::temp_dir().join("denote-test-export-render");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let db_path = fixtures::create_test_bear_db(&dir);
    let reader = BearReader::new(&db_path).unwrap();
    let notes = reader.fetch_notes(false, false).unwrap();

    let project = notes.iter().find(|n| n.id == "NOTE-UUID-AAAA").unwrap();
    let rendered = markdown::render(project, true, &AttachmentMode::Placeholder, None);

    assert!(rendered.starts_with("---\n"));
    assert!(rendered.contains("id: \"NOTE-UUID-AAAA\""));
    assert!(rendered.contains("title: \"Project Notes\""));
    assert!(rendered.contains("tags: [\"work\"]"));
    assert!(rendered.contains("pinned: true"));
    assert!(rendered.contains("This is a test note about the project."));
    // Title heading should be stripped since it duplicates the title
    assert!(!rendered.contains("# Project Notes"));

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn test_full_export_with_filenames() {
    let dir = std::env::temp_dir().join("denote-test-export-full");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let db_path = fixtures::create_test_bear_db(&dir);
    let reader = BearReader::new(&db_path).unwrap();
    let notes = reader.fetch_notes(false, false).unwrap();

    let mut manifest = Manifest::default();
    let strategy = FilenameStrategy::TitleUuid;
    let export = ExportConfig {
        frontmatter: true,
        attachment_mode: AttachmentMode::Placeholder,
        filename_strategy: strategy.clone(),
    };

    let repo_dir = dir.join("repo");
    std::fs::create_dir_all(&repo_dir).unwrap();

    for note in &notes {
        let filename = manifest.generate_filename(note, &export.filename_strategy);
        let content = markdown::render(note, export.frontmatter, &export.attachment_mode, None);
        std::fs::write(repo_dir.join(&filename), content).unwrap();
        manifest.set(note.id.clone(), filename);
    }

    manifest.save(&repo_dir).unwrap();

    assert_eq!(manifest.entries.len(), 3);
    assert!(repo_dir.join(".denote-manifest.json").exists());

    // Verify each note was written
    for (_, filename) in &manifest.entries {
        assert!(repo_dir.join(filename).exists());
    }

    std::fs::remove_dir_all(&dir).ok();
}
