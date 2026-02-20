use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::Shell;

#[derive(Parser)]
#[command(
    name = "git-id",
    version = "1.0.0",
    about = "Manage multiple GitHub accounts on one machine."
)]
pub struct Cli {
    /// Preview changes without modifying any files
    #[arg(long, global = true)]
    pub dry_run: bool,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Add a new account (interactive wizard)
    Add,
    /// List all accounts with status
    List,
    /// Set identity for repo or globally
    Use {
        /// GitHub username (or username@host)
        username: String,
        /// Apply to global git config instead of current repo
        #[arg(long = "global")]
        global: bool,
        /// Convert remote URL to SSH format
        #[arg(long = "ssh")]
        force_ssh: bool,
        /// Convert remote URL to HTTPS format
        #[arg(long = "https")]
        force_https: bool,
    },
    /// Remove an account and its SSH config stanza
    Remove {
        /// GitHub username (or username@host)
        username: String,
        /// Skip confirmation prompt
        #[arg(long, short = 'y')]
        yes: bool,
        /// Also delete the SSH private and public key files
        #[arg(long)]
        delete_keys: bool,
    },
    /// SSH key management subcommands
    Ssh {
        #[command(subcommand)]
        subcommand: SshCommands,
    },
    /// Show current identity and loaded SSH keys
    Status,
    /// Generate shell completion script
    Completions {
        /// Shell to generate completions for
        shell: Shell,
    },
}

#[derive(Subcommand)]
pub enum SshCommands {
    /// Generate a new ed25519 key
    Gen {
        /// GitHub username (or username@host)
        username: String,
    },
    /// Pick an existing ~/.ssh/*.pub key
    Pick {
        /// GitHub username (or username@host)
        username: String,
    },
    /// Write ~/.ssh/config stanzas for all accounts
    Config,
}

/// Build the clap `Command` (used for shell completions).
pub fn build_command() -> clap::Command {
    Cli::command()
}
