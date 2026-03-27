use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "denote", about = "Bear notes backup utility")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,

    /// Path to config file
    #[arg(short, long, global = true)]
    pub config: Option<PathBuf>,

    /// Enable debug logging
    #[arg(short, long, global = true)]
    pub verbose: bool,

    /// Suppress all output except errors
    #[arg(short, long, global = true)]
    pub quiet: bool,
}

#[derive(Subcommand)]
pub enum Command {
    /// Initialize a new denote repo and write default config
    Init {
        /// Path to the git repository
        #[arg(long)]
        repo: PathBuf,

        /// Git remote URL
        #[arg(long)]
        remote: Option<String>,
    },

    /// One-shot: export changed notes, commit, and push
    Sync,

    /// Continuous: monitor Bear's DB and sync on changes
    Watch,

    /// Show last sync time, note count, and repo state
    Status,
}
