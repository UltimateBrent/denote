use serde::Deserialize;
use std::path::{Path, PathBuf};

use crate::errors::{DenoteError, Result};

fn default_bear_db() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/Users/unknown".into());
    PathBuf::from(home).join(
        "Library/Group Containers/9K33E3U3T4.net.shinyfrog.bear/Application Data/database.sqlite",
    )
}

fn default_remote() -> String {
    "origin".into()
}

fn default_branch() -> String {
    "main".into()
}

fn default_push_on_sync() -> bool {
    true
}

fn default_debounce_secs() -> u64 {
    5
}

fn default_commit_template() -> String {
    "denote: synced {count} notes".into()
}

fn default_export_config() -> ExportConfig {
    ExportConfig::default()
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttachmentMode {
    Ignore,
    Placeholder,
    Copy,
}

impl Default for AttachmentMode {
    fn default() -> Self {
        Self::Placeholder
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FilenameStrategy {
    Title,
    Uuid,
    TitleUuid,
}

impl Default for FilenameStrategy {
    fn default() -> Self {
        Self::TitleUuid
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExportConfig {
    #[serde(default = "default_true")]
    pub frontmatter: bool,

    #[serde(default)]
    pub attachment_mode: AttachmentMode,

    #[serde(default)]
    pub filename_strategy: FilenameStrategy,
}

fn default_true() -> bool {
    true
}

impl Default for ExportConfig {
    fn default() -> Self {
        Self {
            frontmatter: true,
            attachment_mode: AttachmentMode::default(),
            filename_strategy: FilenameStrategy::default(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct DenoteConfig {
    #[serde(default = "default_bear_db")]
    pub bear_db: PathBuf,

    pub repo_path: PathBuf,

    #[serde(default = "default_remote")]
    pub remote: String,

    #[serde(default = "default_branch")]
    pub branch: String,

    #[serde(default = "default_push_on_sync")]
    pub push_on_sync: bool,

    #[serde(default = "default_debounce_secs")]
    pub debounce_secs: u64,

    #[serde(default)]
    pub exclude_tags: Vec<String>,

    #[serde(default)]
    pub include_trashed: bool,

    #[serde(default)]
    pub include_archived: bool,

    #[serde(default = "default_commit_template")]
    pub commit_template: String,

    #[serde(default = "default_export_config")]
    pub export: ExportConfig,
}

impl DenoteConfig {
    pub fn load(path: Option<&Path>) -> Result<Self> {
        let config_path = match path {
            Some(p) => p.to_path_buf(),
            None => default_config_path(),
        };

        let mut config: DenoteConfig = if config_path.exists() {
            let contents = std::fs::read_to_string(&config_path).map_err(|e| {
                DenoteError::Config(format!("Failed to read config at {}: {}", config_path.display(), e))
            })?;
            toml::from_str(&contents).map_err(|e| {
                DenoteError::Config(format!("Failed to parse config: {e}"))
            })?
        } else if path.is_some() {
            return Err(DenoteError::Config(format!(
                "Config file not found: {}",
                config_path.display()
            )));
        } else {
            return Err(DenoteError::Config(
                "No config file found. Run `denote init` to create one.".into(),
            ));
        };

        config.apply_env_overrides();
        config.expand_paths();
        config.validate()?;
        Ok(config)
    }

    fn apply_env_overrides(&mut self) {
        if let Ok(v) = std::env::var("DENOTE_BEAR_DB") {
            self.bear_db = PathBuf::from(v);
        }
        if let Ok(v) = std::env::var("DENOTE_REPO_PATH") {
            self.repo_path = PathBuf::from(v);
        }
        if let Ok(v) = std::env::var("DENOTE_REMOTE") {
            self.remote = v;
        }
        if let Ok(v) = std::env::var("DENOTE_BRANCH") {
            self.branch = v;
        }
        if let Ok(v) = std::env::var("DENOTE_PUSH_ON_SYNC") {
            if let Ok(b) = v.parse::<bool>() {
                self.push_on_sync = b;
            }
        }
        if let Ok(v) = std::env::var("DENOTE_DEBOUNCE_SECS") {
            if let Ok(n) = v.parse::<u64>() {
                self.debounce_secs = n;
            }
        }
        if let Ok(v) = std::env::var("DENOTE_INCLUDE_TRASHED") {
            if let Ok(b) = v.parse::<bool>() {
                self.include_trashed = b;
            }
        }
        if let Ok(v) = std::env::var("DENOTE_INCLUDE_ARCHIVED") {
            if let Ok(b) = v.parse::<bool>() {
                self.include_archived = b;
            }
        }
        if let Ok(v) = std::env::var("DENOTE_COMMIT_TEMPLATE") {
            self.commit_template = v;
        }
        if let Ok(v) = std::env::var("DENOTE_EXPORT__FRONTMATTER") {
            if let Ok(b) = v.parse::<bool>() {
                self.export.frontmatter = b;
            }
        }
        if let Ok(v) = std::env::var("DENOTE_EXPORT__ATTACHMENT_MODE") {
            match v.as_str() {
                "ignore" => self.export.attachment_mode = AttachmentMode::Ignore,
                "placeholder" => self.export.attachment_mode = AttachmentMode::Placeholder,
                "copy" => self.export.attachment_mode = AttachmentMode::Copy,
                _ => {}
            }
        }
        if let Ok(v) = std::env::var("DENOTE_EXPORT__FILENAME_STRATEGY") {
            match v.as_str() {
                "title" => self.export.filename_strategy = FilenameStrategy::Title,
                "uuid" => self.export.filename_strategy = FilenameStrategy::Uuid,
                "title-uuid" => self.export.filename_strategy = FilenameStrategy::TitleUuid,
                _ => {}
            }
        }
    }

    fn expand_paths(&mut self) {
        self.bear_db = expand_tilde(&self.bear_db);
        self.repo_path = expand_tilde(&self.repo_path);
    }

    fn validate(&self) -> Result<()> {
        if self.repo_path.as_os_str().is_empty() {
            return Err(DenoteError::Config(
                "repo_path is required".into(),
            ));
        }
        Ok(())
    }

    /// Write a default config to the given path (or the default location).
    pub fn write_default(path: Option<&Path>, repo_path: &Path, remote: Option<&str>) -> Result<PathBuf> {
        let config_path = match path {
            Some(p) => p.to_path_buf(),
            None => default_config_path(),
        };

        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let remote_line = if remote.is_some() {
            "remote = \"origin\"".into()
        } else {
            "# remote = \"origin\"".to_string()
        };

        let contents = format!(
            r#"# denote configuration

# Path to Bear's SQLite database.
# bear_db = "~/Library/Group Containers/9K33E3U3T4.net.shinyfrog.bear/Application Data/database.sqlite"

# Local path to the git repository where notes are exported.
repo_path = "{repo_path}"

# Git remote name and branch to push to.
{remote_line}
branch = "main"

# Whether to push after each commit.
push_on_sync = true

# Seconds to wait after a database change before syncing.
debounce_secs = 5

# Tags to exclude from export.
exclude_tags = []

# Include trashed or archived notes.
include_trashed = false
include_archived = false

# Commit message template. Supports {{count}} placeholder.
commit_template = "denote: synced {{count}} notes"

[export]
frontmatter = true
attachment_mode = "placeholder"
filename_strategy = "title-uuid"
"#,
            repo_path = repo_path.display(),
        );

        std::fs::write(&config_path, contents)?;
        Ok(config_path)
    }
}

fn default_config_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/Users/unknown".into());
    PathBuf::from(home).join(".config/denote/config.toml")
}

pub fn expand_tilde_pub(path: &Path) -> PathBuf {
    expand_tilde(path)
}

fn expand_tilde(path: &Path) -> PathBuf {
    let s = path.to_string_lossy();
    if s.starts_with("~/") {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/Users/unknown".into());
        PathBuf::from(home).join(&s[2..])
    } else {
        path.to_path_buf()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_minimal_config() {
        let dir = std::env::temp_dir().join("denote-test-config");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(f, "repo_path = \"/tmp/notes\"").unwrap();
        drop(f);

        let config = DenoteConfig::load(Some(&path)).unwrap();
        assert_eq!(config.repo_path, PathBuf::from("/tmp/notes"));
        assert_eq!(config.remote, "origin");
        assert_eq!(config.branch, "main");
        assert!(config.push_on_sync);
        assert_eq!(config.debounce_secs, 5);
        assert!(config.export.frontmatter);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_expand_tilde() {
        let p = expand_tilde(Path::new("~/foo/bar"));
        assert!(!p.to_string_lossy().contains('~'));
        assert!(p.to_string_lossy().ends_with("/foo/bar"));
    }

    #[test]
    fn test_missing_explicit_config() {
        let result = DenoteConfig::load(Some(Path::new("/nonexistent/config.toml")));
        assert!(result.is_err());
    }
}
