use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "igp-oracle")]
#[command(about = "Dry-run IGP config reconciliation for Hyperlane registry targets")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Reconcile(ReconcileArgs),
}

#[derive(Debug, Args, Clone)]
pub struct ReconcileArgs {
    #[arg(long)]
    pub config: PathBuf,

    #[arg(long, default_value = ".")]
    pub registry: PathBuf,

    #[arg(long)]
    pub origin: Option<String>,

    #[arg(long)]
    pub remote_chain: Option<String>,

    #[arg(long)]
    pub remote_domain: Option<u32>,

    #[arg(long, default_value = "artifacts")]
    pub output_dir: PathBuf,

    #[arg(long, default_value = "markdown,json")]
    pub format: String,

    #[arg(long)]
    pub dry_run: bool,

    #[arg(long)]
    pub write: bool,

    #[arg(long)]
    pub generate_only: bool,
}

impl ReconcileArgs {
    pub fn is_dry_run(&self) -> bool {
        self.dry_run || !self.write
    }
}
