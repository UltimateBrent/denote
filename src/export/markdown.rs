use regex::Regex;
use std::borrow::Cow;
use std::fmt::Write;
use std::sync::LazyLock;

use crate::config::AttachmentMode;
use crate::db::models::BearNote;

static ATTACHMENT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[(?:image|file):[^\]]*\]").unwrap());

static MD_EMBED_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[([^\]]*)\]\(([^)]+)\)(<!--\s*\{[^}]*\}\s*-->)?").unwrap());

pub fn render(
    note: &BearNote,
    frontmatter: bool,
    attachment_mode: &AttachmentMode,
    assets_subdir: Option<&str>,
) -> String {
    let mut out = String::new();

    if frontmatter {
        write_frontmatter(&mut out, note);
    }

    let body = strip_title_heading(&note.text, &note.title);
    let body = process_attachments(body, attachment_mode, note, assets_subdir);

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

fn process_attachments<'a>(
    text: &'a str,
    mode: &AttachmentMode,
    note: &BearNote,
    assets_subdir: Option<&str>,
) -> Cow<'a, str> {
    match mode {
        AttachmentMode::Ignore => ATTACHMENT_RE.replace_all(text, ""),
        AttachmentMode::Placeholder => Cow::Borrowed(text),
        AttachmentMode::Copy => rewrite_attachment_paths(text, note, assets_subdir),
    }
}

/// For copy mode: rewrite relative image/file references to point into _assets/{note-short-id}/.
fn rewrite_attachment_paths<'a>(
    text: &'a str,
    note: &BearNote,
    assets_subdir: Option<&str>,
) -> Cow<'a, str> {
    if note.attachments.is_empty() {
        return Cow::Borrowed(text);
    }

    let prefix = match assets_subdir {
        Some(sub) => format!("{sub}/"),
        None => {
            let short_id = &note.id[..note.id.len().min(8)];
            format!("_assets/{short_id}/")
        }
    };

    // Build a set of filenames that are known attachments (URL-decoded)
    let attachment_filenames: std::collections::HashSet<String> = note
        .attachments
        .iter()
        .map(|a| a.filename.clone())
        .collect();

    let result = MD_EMBED_RE.replace_all(text, |caps: &regex::Captures| {
        let full_match = caps.get(0).unwrap().as_str();
        let display = &caps[1];
        let raw_path = &caps[2];
        let comment = caps.get(3).map(|m| m.as_str()).unwrap_or("");

        let decoded = percent_decode(raw_path);

        if attachment_filenames.contains(&decoded) {
            let new_path = format!("{}{}", prefix, raw_path);
            let is_image = full_match.starts_with('!');
            if is_image {
                format!("![{display}]({new_path}){comment}")
            } else {
                format!("[{display}]({new_path}){comment}")
            }
        } else {
            full_match.to_string()
        }
    });

    result
}

fn percent_decode(s: &str) -> String {
    let mut result = Vec::new();
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(byte) = u8::from_str_radix(
                &s[i + 1..i + 3],
                16,
            ) {
                result.push(byte);
                i += 3;
                continue;
            }
        }
        result.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&result).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::models::BearAttachment;
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
            attachments: vec![],
        }
    }

    #[test]
    fn test_render_with_frontmatter() {
        let note = sample_note();
        let result = render(&note, true, &AttachmentMode::Placeholder, None);
        assert!(result.starts_with("---\n"));
        assert!(result.contains("id: \"ABC-123\""));
        assert!(result.contains("title: \"Test Note\""));
        assert!(result.contains("tags: [\"work\", \"project\"]"));
        assert!(result.contains("pinned: true"));
        assert!(result.contains("Some content here."));
        assert!(!result.contains("# Test Note"));
    }

    #[test]
    fn test_render_without_frontmatter() {
        let note = sample_note();
        let result = render(&note, false, &AttachmentMode::Placeholder, None);
        assert!(!result.contains("---"));
        assert!(result.contains("Some content here."));
    }

    #[test]
    fn test_strip_attachments() {
        let note = BearNote {
            text: "Before [image:abc/img.png] middle [file:doc.pdf] after".into(),
            ..sample_note()
        };
        let result = render(&note, false, &AttachmentMode::Ignore, None);
        assert!(!result.contains("[image:"));
        assert!(!result.contains("[file:"));
        assert!(result.contains("Before  middle  after"));
    }

    #[test]
    fn test_placeholder_keeps_attachments() {
        let note = BearNote {
            text: "Text ![](image.png) here".into(),
            ..sample_note()
        };
        let result = render(&note, false, &AttachmentMode::Placeholder, None);
        assert!(result.contains("![](image.png)"));
    }

    #[test]
    fn test_copy_rewrites_image_paths() {
        let note = BearNote {
            text: "Before\n![](image.png)<!-- {\"width\":500} -->\nAfter".into(),
            attachments: vec![BearAttachment {
                uuid: "FB84-DEAD".into(),
                filename: "image.png".into(),
                is_image: true,
            }],
            ..sample_note()
        };
        let result = render(&note, false, &AttachmentMode::Copy, None);
        assert!(result.contains("![](_assets/ABC-123/image.png)"));
        assert!(result.contains("<!-- {\"width\":500} -->"));
    }

    #[test]
    fn test_copy_rewrites_file_links() {
        let note = BearNote {
            text: "[Report.pdf](Report.pdf)<!-- {\"embed\":\"true\"} -->".into(),
            attachments: vec![BearAttachment {
                uuid: "DEAD-BEEF".into(),
                filename: "Report.pdf".into(),
                is_image: false,
            }],
            ..sample_note()
        };
        let result = render(&note, false, &AttachmentMode::Copy, None);
        assert!(result.contains("[Report.pdf](_assets/ABC-123/Report.pdf)"));
    }

    #[test]
    fn test_copy_ignores_external_urls() {
        let note = BearNote {
            text: "![logo](https://example.com/logo.png)\n![](image.png)".into(),
            attachments: vec![BearAttachment {
                uuid: "SOME-UUID".into(),
                filename: "image.png".into(),
                is_image: true,
            }],
            ..sample_note()
        };
        let result = render(&note, false, &AttachmentMode::Copy, None);
        assert!(result.contains("![logo](https://example.com/logo.png)"));
        assert!(result.contains("![](_assets/ABC-123/image.png)"));
    }

    #[test]
    fn test_copy_handles_url_encoded_filenames() {
        let note = BearNote {
            text: "![](Screenshot%202023-10-10%20at%201.20.png)".into(),
            attachments: vec![BearAttachment {
                uuid: "SOME-UUID".into(),
                filename: "Screenshot 2023-10-10 at 1.20.png".into(),
                is_image: true,
            }],
            ..sample_note()
        };
        let result = render(&note, false, &AttachmentMode::Copy, None);
        assert!(result.contains("![](_assets/ABC-123/Screenshot%202023-10-10%20at%201.20.png)"));
    }
}
