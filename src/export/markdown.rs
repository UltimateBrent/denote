use regex::Regex;
use std::fmt::Write;
use std::sync::LazyLock;

use crate::config::AttachmentMode;
use crate::db::models::BearNote;

static ATTACHMENT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[(?:image|file):[^\]]*\]").unwrap());

pub fn render(note: &BearNote, frontmatter: bool, attachment_mode: &AttachmentMode) -> String {
    let mut out = String::new();

    if frontmatter {
        write_frontmatter(&mut out, note);
    }

    let body = strip_title_heading(&note.text, &note.title);
    let body = process_attachments(body, attachment_mode);

    out.push_str(&body);

    if !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

fn write_frontmatter(out: &mut String, note: &BearNote) {
    let created = note
        .created
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_default();
    let modified = note
        .modified
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_default();

    out.push_str("---\n");
    writeln!(out, "id: \"{}\"", note.id).unwrap();
    writeln!(out, "title: \"{}\"", escape_yaml(&note.title)).unwrap();

    write!(out, "tags: [").unwrap();
    for (i, tag) in note.tags.iter().enumerate() {
        if i > 0 {
            write!(out, ", ").unwrap();
        }
        write!(out, "\"{}\"", escape_yaml(tag)).unwrap();
    }
    writeln!(out, "]").unwrap();

    writeln!(out, "created: \"{}\"", created).unwrap();
    writeln!(out, "modified: \"{}\"", modified).unwrap();
    if note.is_pinned {
        writeln!(out, "pinned: true").unwrap();
    }
    out.push_str("---\n\n");
}

fn escape_yaml(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Bear includes the note title as `# Title` at the start of the text body.
/// Strip it to avoid duplication with the frontmatter title field.
fn strip_title_heading<'a>(text: &'a str, title: &str) -> &'a str {
    let trimmed = text.trim_start();
    if let Some(rest) = trimmed.strip_prefix("# ") {
        let first_line_end = rest.find('\n').unwrap_or(rest.len());
        let heading_text = rest[..first_line_end].trim();
        if heading_text == title.trim() {
            let after = &rest[first_line_end..];
            return after.strip_prefix('\n').unwrap_or(after);
        }
    }
    text
}

fn process_attachments<'a>(text: &'a str, mode: &AttachmentMode) -> std::borrow::Cow<'a, str> {
    match mode {
        AttachmentMode::Ignore => ATTACHMENT_RE.replace_all(text, ""),
        AttachmentMode::Placeholder | AttachmentMode::Copy => std::borrow::Cow::Borrowed(text),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::OffsetDateTime;

    fn sample_note() -> BearNote {
        BearNote {
            id: "ABC-123".into(),
            title: "Test Note".into(),
            text: "# Test Note\n\nSome content here.\n".into(),
            tags: vec!["work".into(), "project".into()],
            created: OffsetDateTime::UNIX_EPOCH,
            modified: OffsetDateTime::UNIX_EPOCH,
            is_trashed: false,
            is_archived: false,
            is_pinned: true,
        }
    }

    #[test]
    fn test_render_with_frontmatter() {
        let note = sample_note();
        let result = render(&note, true, &AttachmentMode::Placeholder);
        assert!(result.starts_with("---\n"));
        assert!(result.contains("id: \"ABC-123\""));
        assert!(result.contains("title: \"Test Note\""));
        assert!(result.contains("tags: [\"work\", \"project\"]"));
        assert!(result.contains("pinned: true"));
        assert!(result.contains("Some content here."));
        // Title heading should be stripped
        assert!(!result.contains("# Test Note"));
    }

    #[test]
    fn test_render_without_frontmatter() {
        let note = sample_note();
        let result = render(&note, false, &AttachmentMode::Placeholder);
        assert!(!result.contains("---"));
        assert!(result.contains("Some content here."));
    }

    #[test]
    fn test_strip_attachments() {
        let note = BearNote {
            text: "Before [image:abc/img.png] middle [file:doc.pdf] after".into(),
            ..sample_note()
        };
        let result = render(&note, false, &AttachmentMode::Ignore);
        assert!(!result.contains("[image:"));
        assert!(!result.contains("[file:"));
        assert!(result.contains("Before  middle  after"));
    }

    #[test]
    fn test_placeholder_keeps_attachments() {
        let note = BearNote {
            text: "Text [image:abc/img.png] here".into(),
            ..sample_note()
        };
        let result = render(&note, false, &AttachmentMode::Placeholder);
        assert!(result.contains("[image:abc/img.png]"));
    }
}
