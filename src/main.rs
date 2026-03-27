mod cli;

use clap::Parser;
use tracing::error;
use tracing_subscriber::EnvFilter;

use cli::{Cli, Command};
use denote::config::{self, DenoteConfig};
use denote::errors::Result;
use denote::{git, watcher};

fn main() {
    let cli = Cli::parse();
    init_tracing(cli.verbose, cli.quiet);

    if let Err(e) = run(cli) {
        error!(error = %e, "Fatal error");
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Command::Init { repo, remote } => {
            let repo_path = config::expand_tilde_pub(&repo);
            git::init_repo(&repo_path, remote.as_deref())?;
            let config_path =
                DenoteConfig::write_default(cli.config.as_deref(), &repo_path, remote.as_deref())?;
            println!("Initialized repo at {}", repo_path.display());
            println!("Config written to {}", config_path.display());
            Ok(())
        }
        Command::Sync => {
            let config = DenoteConfig::load(cli.config.as_deref())?;
            let count = watcher::sync_cycle(&config)?;
            println!("Synced {count} notes");
            Ok(())
        }
        Command::Watch => {
            let config = DenoteConfig::load(cli.config.as_deref())?;
            watcher::watch(&config)
        }
        Command::Status => {
            let config = DenoteConfig::load(cli.config.as_deref())?;
            let status = git::repo_status(&config.repo_path)?;
            println!("denote status");
            println!("─────────────");
            if let Some(time) = &status.head_commit_time {
                println!("Last sync:    {time}");
            } else {
                println!("Last sync:    never");
            }
            if let Some(msg) = &status.head_commit_message {
                println!("Last commit:  {msg}");
            }
            println!("Exported:     {} notes", status.file_count);
            if status.is_dirty {
                println!("Repo state:   uncommitted changes");
            } else {
                println!("Repo state:   clean");
            }
            Ok(())
        }
    }
}

fn init_tracing(verbose: bool, quiet: bool) {
    let filter = if quiet {
        "error"
    } else if verbose {
        "denote=debug"
    } else {
        "denote=info"
    };

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(filter)),
        )
        .with_target(false)
        .init();
}
