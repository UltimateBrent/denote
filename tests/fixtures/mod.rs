use rusqlite::Connection;
use std::path::{Path, PathBuf};

/// Creates a minimal SQLite database mimicking Bear's Core Data schema.
/// Returns the path to the temporary database file.
pub fn create_test_bear_db(dir: &Path) -> PathBuf {
    let db_path = dir.join("database.sqlite");
    let conn = Connection::open(&db_path).unwrap();

    conn.execute_batch(
        "
        CREATE TABLE ZSFNOTE (
            Z_PK INTEGER PRIMARY KEY,
            ZUNIQUEIDENTIFIER TEXT NOT NULL,
            ZTITLE TEXT,
            ZTEXT TEXT,
            ZCREATIONDATE REAL,
            ZMODIFICATIONDATE REAL,
            ZPINNED INTEGER DEFAULT 0,
            ZTRASHED INTEGER DEFAULT 0,
            ZARCHIVED INTEGER DEFAULT 0
        );

        CREATE TABLE ZSFNOTETAG (
            Z_PK INTEGER PRIMARY KEY,
            ZTITLE TEXT NOT NULL
        );

        -- Junction table with Bear's auto-numbered naming convention
        CREATE TABLE Z_5TAGS (
            Z_5NOTES INTEGER,
            Z_13TAGS INTEGER,
            FOREIGN KEY (Z_5NOTES) REFERENCES ZSFNOTE(Z_PK),
            FOREIGN KEY (Z_13TAGS) REFERENCES ZSFNOTETAG(Z_PK)
        );
        ",
    )
    .unwrap();

    // Core Data epoch is 2001-01-01. Offset from Unix epoch = 978307200.
    // 2026-03-15 10:30:00 UTC = Unix 1773750600 => Core Data = 1773750600 - 978307200 = 795443400
    // 2026-03-27 14:22:00 UTC = Unix 1774812120 => Core Data = 1774812120 - 978307200 = 796504920
    let created_ts: f64 = 795443400.0;
    let modified_ts: f64 = 796504920.0;

    conn.execute(
        "INSERT INTO ZSFNOTE (Z_PK, ZUNIQUEIDENTIFIER, ZTITLE, ZTEXT, ZCREATIONDATE, ZMODIFICATIONDATE, ZPINNED, ZTRASHED, ZARCHIVED)
         VALUES (1, 'NOTE-UUID-AAAA', 'Project Notes', '# Project Notes\n\nThis is a test note about the project.\n', ?1, ?2, 1, 0, 0)",
        rusqlite::params![created_ts, modified_ts],
    ).unwrap();

    conn.execute(
        "INSERT INTO ZSFNOTE (Z_PK, ZUNIQUEIDENTIFIER, ZTITLE, ZTEXT, ZCREATIONDATE, ZMODIFICATIONDATE, ZPINNED, ZTRASHED, ZARCHIVED)
         VALUES (2, 'NOTE-UUID-BBBB', 'Weekly Standup', '# Weekly Standup\n\nMonday: reviewed PRs\nTuesday: shipped feature\n', ?1, ?2, 0, 0, 0)",
        rusqlite::params![created_ts, modified_ts],
    ).unwrap();

    conn.execute(
        "INSERT INTO ZSFNOTE (Z_PK, ZUNIQUEIDENTIFIER, ZTITLE, ZTEXT, ZCREATIONDATE, ZMODIFICATIONDATE, ZPINNED, ZTRASHED, ZARCHIVED)
         VALUES (3, 'NOTE-UUID-CCCC', 'Private Stuff', '# Private Stuff\n\nSecret content.\n', ?1, ?2, 0, 0, 0)",
        rusqlite::params![created_ts, modified_ts],
    ).unwrap();

    conn.execute(
        "INSERT INTO ZSFNOTE (Z_PK, ZUNIQUEIDENTIFIER, ZTITLE, ZTEXT, ZCREATIONDATE, ZMODIFICATIONDATE, ZPINNED, ZTRASHED, ZARCHIVED)
         VALUES (4, 'NOTE-UUID-DDDD', 'Trashed Note', '# Trashed Note\n\nIn the trash.\n', ?1, ?2, 0, 1, 0)",
        rusqlite::params![created_ts, modified_ts],
    ).unwrap();

    // Tags
    conn.execute_batch(
        "
        INSERT INTO ZSFNOTETAG (Z_PK, ZTITLE) VALUES (1, 'work');
        INSERT INTO ZSFNOTETAG (Z_PK, ZTITLE) VALUES (2, 'meeting');
        INSERT INTO ZSFNOTETAG (Z_PK, ZTITLE) VALUES (3, 'private');
        ",
    )
    .unwrap();

    // Note-tag associations
    conn.execute_batch(
        "
        INSERT INTO Z_5TAGS (Z_5NOTES, Z_13TAGS) VALUES (1, 1);  -- Project Notes -> work
        INSERT INTO Z_5TAGS (Z_5NOTES, Z_13TAGS) VALUES (2, 1);  -- Weekly Standup -> work
        INSERT INTO Z_5TAGS (Z_5NOTES, Z_13TAGS) VALUES (2, 2);  -- Weekly Standup -> meeting
        INSERT INTO Z_5TAGS (Z_5NOTES, Z_13TAGS) VALUES (3, 3);  -- Private Stuff -> private
        ",
    )
    .unwrap();

    db_path
}
