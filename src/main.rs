mod cli;
mod commands;
mod config;
mod git;
mod models;
mod ssh;
mod ui;

use cli::{Cli, Commands, SshCommands};
use clap::Parser;

fn main() {
    let cli = Cli::parse();
    let dry_run = cli.dry_run;

    match cli.command {
        Commands::Add => commands::add::cmd_add(dry_run),
        Commands::List => commands::list::cmd_list(),
        Commands::Use { username, global, force_ssh, force_https } => {
            commands::use_cmd::cmd_use(&username, global, force_ssh, force_https, dry_run);
        }
        Commands::Remove { username, yes, delete_keys } => {
            commands::remove::cmd_remove(&username, yes, delete_keys, dry_run);
        }
        Commands::Ssh { subcommand } => match subcommand {
            SshCommands::Gen { username } => commands::ssh::cmd_ssh_gen(&username, dry_run),
            SshCommands::Pick { username } => commands::ssh::cmd_ssh_pick(&username, dry_run),
            SshCommands::Config => commands::ssh::cmd_ssh_config(dry_run),
        },
        Commands::Status => commands::status::cmd_status(),
        Commands::Completions { shell } => commands::completions::cmd_completions(shell),
    }
}
