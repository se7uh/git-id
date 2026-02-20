use crate::cli::build_command;
use clap_complete::{generate, Shell};
use std::io;

pub fn cmd_completions(shell: Shell) {
    let mut cmd = build_command();
    generate(shell, &mut cmd, "git-id", &mut io::stdout());
}
