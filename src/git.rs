use std::path::Path;

use git2::{Cred, Direction, PushOptions, RemoteCallbacks, Repository, Signature};
use tracing::{debug, info, warn};

use crate::errors::{DenoteError, Result};

pub fn init_repo(path: &Path, remote_url: Option<&str>) -> Result<()> {
    std::fs::create_dir_all(path)?;

    let mut opts = git2::RepositoryInitOptions::new();
    opts.initial_head("main");
    let repo = Repository::init_opts(path, &opts)?;
    info!(path = %path.display(), "Initialized git repository");

    if let Some(url) = remote_url {
        repo.remote("origin", url)?;
        info!(url, "Added remote 'origin'");
    }

    Ok(())
}

pub fn commit_and_push(
    repo_path: &Path,
    message: &str,
    remote_name: &str,
    branch: &str,
    push: bool,
) -> Result<()> {
    let repo = Repository::open(repo_path)?;

    let mut index = repo.index()?;
    index.add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)?;
    index.write()?;

    let tree_oid = index.write_tree()?;
    let tree = repo.find_tree(tree_oid)?;

    if repo.head().is_ok() {
        let head = repo.head()?;
        let parent = head.peel_to_commit()?;

        // Skip commit if tree is identical to parent (no changes)
        if parent.tree()?.id() == tree_oid {
            debug!("No changes to commit");
            return Ok(());
        }

        let sig = get_signature(&repo)?;
        repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &[&parent])?;
    } else {
        let sig = get_signature(&repo)?;
        repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &[])?;
    };

    info!(message, "Created commit");

    if push {
        push_to_remote(&repo, remote_name, branch)?;
    }

    Ok(())
}

fn get_signature(repo: &Repository) -> Result<Signature<'static>> {
    match repo.signature() {
        Ok(sig) => Ok(Signature::now(
            &sig.name().unwrap_or("denote"),
            &sig.email().unwrap_or("denote@localhost"),
        )?),
        Err(_) => Ok(Signature::now("denote", "denote@localhost")?),
    }
}

fn push_to_remote(repo: &Repository, remote_name: &str, branch: &str) -> Result<()> {
    let mut remote = repo.find_remote(remote_name).map_err(|e| {
        DenoteError::Git(git2::Error::from_str(&format!(
            "Remote '{}' not found: {}",
            remote_name, e
        )))
    })?;

    let mut callbacks = RemoteCallbacks::new();

    callbacks.credentials(|_url, username_from_url, cred_type| {
        if cred_type.contains(git2::CredentialType::SSH_KEY) {
            let user = username_from_url.unwrap_or("git");
            Cred::ssh_key_from_agent(user)
        } else if cred_type.contains(git2::CredentialType::USER_PASS_PLAINTEXT) {
            Cred::credential_helper(&git2::Config::open_default()?, _url, username_from_url)
        } else {
            Cred::default()
        }
    });

    callbacks.push_update_reference(|refname, status| {
        if let Some(msg) = status {
            warn!(refname, msg, "Push rejected");
        } else {
            debug!(refname, "Push accepted");
        }
        Ok(())
    });

    let mut push_opts = PushOptions::new();
    push_opts.remote_callbacks(callbacks);

    let refspec = format!("refs/heads/{branch}:refs/heads/{branch}");

    // Connect first to validate credentials
    let mut conn_callbacks = RemoteCallbacks::new();
    conn_callbacks.credentials(|_url, username_from_url, cred_type| {
        if cred_type.contains(git2::CredentialType::SSH_KEY) {
            let user = username_from_url.unwrap_or("git");
            Cred::ssh_key_from_agent(user)
        } else if cred_type.contains(git2::CredentialType::USER_PASS_PLAINTEXT) {
            Cred::credential_helper(&git2::Config::open_default()?, _url, username_from_url)
        } else {
            Cred::default()
        }
    });

    remote.connect_auth(Direction::Push, Some(conn_callbacks), None)?;
    remote.disconnect()?;

    remote.push(&[&refspec], Some(&mut push_opts))?;
    info!(remote = remote_name, branch, "Pushed to remote");

    Ok(())
}

/// Describe the current repo state for `denote status`.
pub struct RepoStatus {
    pub head_commit_message: Option<String>,
    pub head_commit_time: Option<String>,
    pub file_count: usize,
    pub is_dirty: bool,
}

pub fn repo_status(repo_path: &Path) -> Result<RepoStatus> {
    let repo = Repository::open(repo_path)?;

    let (head_commit_message, head_commit_time) = match repo.head() {
        Ok(head) => {
            let commit = head.peel_to_commit()?;
            let msg = commit.message().map(|m| m.trim().to_string());
            let time = commit.time();
            let secs = time.seconds();
            let ts = time::OffsetDateTime::from_unix_timestamp(secs)
                .map(|t| {
                    t.format(&time::format_description::well_known::Rfc3339)
                        .unwrap_or_default()
                })
                .unwrap_or_default();
            (msg, Some(ts))
        }
        Err(_) => (None, None),
    };

    let file_count = std::fs::read_dir(repo_path)
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .filter(|e| {
                    e.path()
                        .extension()
                        .map(|ext| ext == "md")
                        .unwrap_or(false)
                })
                .count()
        })
        .unwrap_or(0);

    let statuses = repo.statuses(None)?;
    let is_dirty = !statuses.is_empty();

    Ok(RepoStatus {
        head_commit_message,
        head_commit_time,
        file_count,
        is_dirty,
    })
}
