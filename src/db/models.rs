use std::path::PathBuf;

use time::OffsetDateTime;

#[derive(Debug, Clone)]
pub struct BearAttachment {
    /// UUID of the attachment (used as folder name in Bear's Local Files)
    pub uuid: String,
    /// Original filename as stored by Bear (e.g. "image.png", "report.pdf")
    pub filename: String,
    /// Whether this is an image (vs a generic file)
    pub is_image: bool,
}

impl BearAttachment {
    /// Resolve the full path to this attachment in Bear's local storage.
    pub fn source_path(&self, bear_db: &std::path::Path) -> PathBuf {
        let local_files = bear_db
            .parent()
            .unwrap_or(std::path::Path::new("."))
            .join("Local Files");
        let subdir = if self.is_image {
            "Note Images"
        } else {
            "Note Files"
        };
        local_files.join(subdir).join(&self.uuid).join(&self.filename)
    }
}

#[derive(Debug, Clone)]
pub struct BearNote {
    pub id: String,
    pub title: String,
    pub text: String,
    pub tags: Vec<String>,
    #[allow(dead_code)]
    pub created: OffsetDateTime,
    pub modified: OffsetDateTime,
    pub is_trashed: bool,
    pub is_archived: bool,
    pub is_pinned: bool,
    pub attachments: Vec<BearAttachment>,
}
