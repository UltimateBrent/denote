use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags};

use crate::db::models::BearNote;
use crate::errors::{DenoteError, Result};

pub trait NoteSource {
    fn fetch_notes(&self, include_trashed: bool, include_archived: bool) -> Result<Vec<BearNote>>;
}

/// Schema metadata discovered at connect time.
struct SchemaInfo {
    junction_table: String,
    note_col: String,
    tag_col: String,
}

pub struct BearReader {
    db_path: PathBuf,
}

impl BearReader {
    pub fn new(db_path: &Path) -> Result<Self> {
        if !db_path.exists() {
            return Err(DenoteError::DbNotFound(db_path.to_path_buf()));
        }
        Ok(Self {
            db_path: db_path.to_path_buf(),
        })
    }

    fn open_connection(&self) -> Result<Connection> {
        let conn = Connection::open_with_flags(
            &self.db_path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        conn.execute_batch("PRAGMA query_only = ON; PRAGMA busy_timeout = 5000;")?;
        Ok(conn)
    }

    fn discover_schema(conn: &Connection) -> Result<SchemaInfo> {
        let mut stmt = conn.prepare(
            "SELECT name FROM sqlite_master WHERE type = 'table' AND name LIKE 'Z_%TAGS'",
        )?;

        let table_names: Vec<String> = stmt
            .query_map([], |row| row.get(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        for table_name in &table_names {
            let pragma_sql = format!("PRAGMA table_info(\"{}\")", table_name);
            let mut pragma_stmt = conn.prepare(&pragma_sql)?;

            let columns: Vec<String> = pragma_stmt
                .query_map([], |row| row.get::<_, String>(1))?
                .collect::<std::result::Result<Vec<_>, _>>()?;

            let note_col = columns.iter().find(|c| {
                let upper = c.to_uppercase();
                upper.starts_with("Z_") && upper.ends_with("NOTES")
            });
            let tag_col = columns.iter().find(|c| {
                let upper = c.to_uppercase();
                upper.starts_with("Z_") && upper.ends_with("TAGS")
            });

            if let (Some(nc), Some(tc)) = (note_col, tag_col) {
                return Ok(SchemaInfo {
                    junction_table: table_name.clone(),
                    note_col: nc.clone(),
                    tag_col: tc.clone(),
                });
            }
        }

        Err(DenoteError::Config(
            "Could not discover Bear's junction table schema. Is this a valid Bear database?".into(),
        ))
    }
}

const CORE_DATA_EPOCH_OFFSET: i64 = 978307200;

const IMAGE_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "heic", "webp", "tiff", "tif", "bmp", "svg",
];

use crate::db::models::BearAttachment;

impl BearReader {
    /// Load all attachments keyed by note Z_PK.
    fn load_attachments(
        conn: &Connection,
    ) -> Result<std::collections::HashMap<i64, Vec<BearAttachment>>> {
        let mut stmt = conn.prepare(
            "SELECT ZNOTE, ZUNIQUEIDENTIFIER, ZFILENAME, ZNORMALIZEDFILEEXTENSION FROM ZSFNOTEFILE WHERE ZPERMANENTLYDELETED = 0 AND ZNOTE IS NOT NULL",
        )?;

        let mut map: std::collections::HashMap<i64, Vec<BearAttachment>> =
            std::collections::HashMap::new();

        let rows = stmt.query_map([], |row| {
            let note_pk: i64 = row.get(0)?;
            let uuid: String = row.get(1)?;
            let filename: String = row.get(2)?;
            let ext: Option<String> = row.get(3)?;
            Ok((note_pk, uuid, filename, ext))
        })?;

        for row in rows {
            let (note_pk, uuid, filename, ext) = row?;
            let is_image = ext
                .as_deref()
                .map(|e| IMAGE_EXTENSIONS.contains(&e.to_lowercase().as_str()))
                .unwrap_or(false);
            map.entry(note_pk)
                .or_default()
                .push(BearAttachment { uuid, filename, is_image });
        }

        Ok(map)
    }
}

impl NoteSource for BearReader {
    fn fetch_notes(&self, include_trashed: bool, include_archived: bool) -> Result<Vec<BearNote>> {
        let conn = self.open_connection()?;
        let schema = Self::discover_schema(&conn)?;
        let attachments_map = Self::load_attachments(&conn)?;

        let mut where_clauses = Vec::new();
        if !include_trashed {
            where_clauses.push("n.ZTRASHED = 0");
        }
        if !include_archived {
            where_clauses.push("n.ZARCHIVED = 0");
        }

        let where_sql = if where_clauses.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", where_clauses.join(" AND "))
        };

        let sql = format!(
            r#"
            SELECT
                n.ZUNIQUEIDENTIFIER,
                COALESCE(n.ZTITLE, ''),
                COALESCE(n.ZTEXT, ''),
                datetime(n.ZCREATIONDATE + {epoch}, 'unixepoch'),
                datetime(n.ZMODIFICATIONDATE + {epoch}, 'unixepoch'),
                COALESCE(n.ZTRASHED, 0),
                COALESCE(n.ZARCHIVED, 0),
                COALESCE(n.ZPINNED, 0),
                GROUP_CONCAT(t.ZTITLE, '||'),
                n.Z_PK
            FROM ZSFNOTE n
            LEFT JOIN {jt} jt ON jt.{note_col} = n.Z_PK
            LEFT JOIN ZSFNOTETAG t ON t.Z_PK = jt.{tag_col}
            {where_sql}
            GROUP BY n.ZUNIQUEIDENTIFIER
            ORDER BY n.ZMODIFICATIONDATE DESC
            "#,
            epoch = CORE_DATA_EPOCH_OFFSET,
            jt = schema.junction_table,
            note_col = schema.note_col,
            tag_col = schema.tag_col,
            where_sql = where_sql,
        );

        let mut stmt = conn.prepare(&sql)?;
        let notes = stmt
            .query_map([], |row| {
                let id: String = row.get(0)?;
                let title: String = row.get(1)?;
                let text: String = row.get(2)?;
                let created_str: String = row.get(3)?;
                let modified_str: String = row.get(4)?;
                let is_trashed: i32 = row.get(5)?;
                let is_archived: i32 = row.get(6)?;
                let is_pinned: i32 = row.get(7)?;
                let tags_raw: Option<String> = row.get(8)?;
                let z_pk: i64 = row.get(9)?;

                let tags: Vec<String> = tags_raw
                    .map(|s| s.split("||").map(|t| t.to_string()).collect())
                    .unwrap_or_default();

                Ok((
                    id,
                    title,
                    text,
                    created_str,
                    modified_str,
                    is_trashed != 0,
                    is_archived != 0,
                    is_pinned != 0,
                    tags,
                    z_pk,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        notes
            .into_iter()
            .map(|(id, title, text, created_str, modified_str, is_trashed, is_archived, is_pinned, tags, z_pk)| {
                let created = parse_sqlite_datetime(&created_str).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(3, rusqlite::types::Type::Text, Box::new(e))
                })?;
                let modified = parse_sqlite_datetime(&modified_str).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(4, rusqlite::types::Type::Text, Box::new(e))
                })?;

                let attachments = attachments_map.get(&z_pk).cloned().unwrap_or_default();

                Ok(BearNote {
                    id,
                    title,
                    text,
                    tags,
                    created,
                    modified,
                    is_trashed,
                    is_archived,
                    is_pinned,
                    attachments,
                })
            })
            .collect::<Result<Vec<_>>>()
    }
}

fn parse_sqlite_datetime(s: &str) -> std::result::Result<time::OffsetDateTime, time::error::Parse> {
    let format =
        time::format_description::parse("[year]-[month]-[day] [hour]:[minute]:[second]").unwrap();
    let pdt = time::PrimitiveDateTime::parse(s, &format)?;
    Ok(pdt.assume_utc())
}
