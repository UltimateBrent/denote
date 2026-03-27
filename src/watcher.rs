use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;

use notify_debouncer_full::{new_debouncer, DebounceEventResult};
use tracing::{debug, error, info};

use crate::config::DenoteConfig;
use crate::db::reader::{BearReader, NoteSource};
use crate::errors::Result;
use crate::export::diff;
use crate::export::filemap::Manifest;
use crate::git;

/// Run a single sync cycle: read notes, diff, write, commit, push.
pub fn sync_cycle(config: &DenoteConfig) -> Result<usize> {
    let reader = BearReader::new(&config.bear_db)?;
    let notes = reader.fetch_notes(config.include_trashed, config.include_archived)?;
    debug!(count = notes.len(), "Read notes from Bear");

    let mut manifest = Manifest::load(&config.repo_path)?;
    let changes = diff::compute_diff(
        &notes,
        &manifest,
        &config.repo_path,
        &config.exclude_tags,
        &config.export,
    );

    if changes.is_empty() {
        debug!("No changes detected");
        return Ok(0);
    }

    info!(changes = changes.len(), "Applying changes");
    let count = diff::apply_changes(
        &changes,
        &mut manifest,
        &config.repo_path,
        &config.export,
        Some(&config.bear_db),
    )?;

    let message = config
        .commit_template
        .replace("{count}", &count.to_string());
    git::commit_and_push(
        &config.repo_path,
        &message,
        &config.remote,
        &config.branch,
        config.push_on_sync,
    )?;

    Ok(count)
}

/// Start watching Bear's database and sync on changes.
pub fn watch(config: &DenoteConfig) -> Result<()> {
    info!(
        bear_db = %config.bear_db.display(),
        repo = %config.repo_path.display(),
        debounce = config.debounce_secs,
        "Starting watcher"
    );

    // Initial sync on startup
    match sync_cycle(config) {
        Ok(count) => info!(count, "Initial sync complete"),
        Err(e) => error!(error = %e, "Initial sync failed"),
    }

    let (tx, rx) = mpsc::channel();

    let mut debouncer = new_debouncer(
        Duration::from_secs(config.debounce_secs),
        None,
        move |result: DebounceEventResult| match result {
            Ok(events) => {
                debug!(count = events.len(), "Debounced file events");
                let _ = tx.send(());
            }
            Err(errors) => {
                for e in errors {
                    error!(error = %e, "Watcher error");
                }
            }
        },
    )?;

    let db_path = &config.bear_db;
    let watch_dir = db_path.parent().unwrap_or(Path::new("."));

    debouncer
        .watch(watch_dir, notify::RecursiveMode::NonRecursive)?;
    info!(path = %watch_dir.display(), "Watching for changes");

    loop {
        match rx.recv() {
            Ok(()) => {
                debug!("Change detected, running sync");
                match sync_cycle(config) {
                    Ok(0) => debug!("No changes in this cycle"),
                    Ok(count) => info!(count, "Sync cycle complete"),
                    Err(e) => error!(error = %e, "Sync cycle failed"),
                }
            }
            Err(e) => {
                error!(error = %e, "Watcher channel closed");
                break;
            }
        }
    }

    Ok(())
}
