use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

use crate::config::FilenameStrategy;
use crate::db::models::BearNote;
use crate::errors::Result;

const MANIFEST_FILENAME: &str = ".denote-manifest.json";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Manifest {
    #[serde(flatten)]
    pub entries: HashMap<String, String>,
}

impl Manifest {
    pub fn load(repo_path: &Path) -> Result<Self> {
        let path = repo_path.join(MANIFEST_FILENAME);
        if !path.exists() {
            return Ok(Self::default());
        }
        let contents = std::fs::read_to_string(&path)?;
        let manifest: Self = serde_json::from_str(&contents)?;
        Ok(manifest)
    }

    pub fn save(&self, repo_path: &Path) -> Result<()> {
        let path = repo_path.join(MANIFEST_FILENAME);
        let contents = serde_json::to_string_pretty(&self.entries)?;
        std::fs::write(&path, contents)?;
        Ok(())
    }

    pub fn filename_for(&self, uuid: &str) -> Option<&String> {
        self.entries.get(uuid)
    }

    pub fn set(&mut self, uuid: String, filename: String) {
        self.entries.insert(uuid, filename);
    }

    pub fn remove(&mut self, uuid: &str) -> Option<String> {
        self.entries.remove(uuid)
    }

    /// Generate a filename for a note, handling collisions with existing entries.
    pub fn generate_filename(
        &self,
        note: &BearNote,
        strategy: &FilenameStrategy,
    ) -> String {
        let base = match strategy {
            FilenameStrategy::Title => slug::slugify(&note.title),
            FilenameStrategy::Uuid => note.id.clone(),
            FilenameStrategy::TitleUuid => {
                let title_slug = slug::slugify(&note.title);
                let short_uuid = note.id[..note.id.len().min(6)].to_lowercase();
                format!("{title_slug}-{short_uuid}")
            }
        };

        let base = if base.is_empty() {
            "untitled".to_string()
        } else {
            base
        };

        let existing_filenames: std::collections::HashSet<&str> = self
            .entries
            .iter()
            .filter(|(uuid, _)| uuid.as_str() != note.id)
            .map(|(_, fname)| fname.as_str())
            .collect();

        let candidate = format!("{base}.md");
        if !existing_filenames.contains(candidate.as_str()) {
            return candidate;
        }

        for i in 2.. {
            let candidate = format!("{base}-{i}.md");
            if !existing_filenames.contains(candidate.as_str()) {
                return candidate;
            }
        }

        unreachable!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::OffsetDateTime;

    fn note(id: &str, title: &str) -> BearNote {
        BearNote {
            id: id.into(),
            title: title.into(),
            text: String::new(),
            tags: vec![],
            created: OffsetDateTime::UNIX_EPOCH,
            modified: OffsetDateTime::UNIX_EPOCH,
            is_trashed: false,
            is_archived: false,
            is_pinned: false,
            attachments: vec![],
        }
    }

    #[test]
    fn test_title_strategy() {
        let m = Manifest::default();
        let n = note("ABC-123", "My Project Notes");
        let f = m.generate_filename(&n, &FilenameStrategy::Title);
        assert_eq!(f, "my-project-notes.md");
    }

    #[test]
    fn test_uuid_strategy() {
        let m = Manifest::default();
        let n = note("ABC-123", "My Project Notes");
        let f = m.generate_filename(&n, &FilenameStrategy::Uuid);
        assert_eq!(f, "ABC-123.md");
    }

    #[test]
    fn test_title_uuid_strategy() {
        let m = Manifest::default();
        let n = note("ABCDEF-123456", "My Project Notes");
        let f = m.generate_filename(&n, &FilenameStrategy::TitleUuid);
        assert_eq!(f, "my-project-notes-abcdef.md");
    }

    #[test]
    fn test_collision_handling() {
        let mut m = Manifest::default();
        m.set("OTHER-ID".into(), "my-notes.md".into());

        let n = note("NEW-ID", "My Notes");
        let f = m.generate_filename(&n, &FilenameStrategy::Title);
        assert_eq!(f, "my-notes-2.md");
    }

    #[test]
    fn test_empty_title() {
        let m = Manifest::default();
        let n = note("ABC-123", "");
        let f = m.generate_filename(&n, &FilenameStrategy::Title);
        assert_eq!(f, "untitled.md");
    }

    #[test]
    fn test_same_uuid_no_collision() {
        let mut m = Manifest::default();
        m.set("ABC-123".into(), "my-notes.md".into());

        let n = note("ABC-123", "My Notes");
        let f = m.generate_filename(&n, &FilenameStrategy::Title);
        // Same UUID should not collide with itself
        assert_eq!(f, "my-notes.md");
    }
}
