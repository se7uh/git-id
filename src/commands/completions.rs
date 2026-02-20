use crate::cli::build_command;
use clap_complete::{generate, Shell};
use std::fs;
use std::io::Write;
use std::path::PathBuf;

pub fn cmd_completions(shell: Shell) {
    let mut cmd = build_command();

    let home = dirs::home_dir().expect("Could not determine home directory");

    match shell {
        Shell::Zsh => setup_zsh(&mut cmd, &home),
        Shell::Bash => setup_bash(&mut cmd, &home),
        Shell::Fish => {
            let path = home.join(".config/fish/completions/git-id.fish");
            write_completion(shell, &mut cmd, &path);
            println!("✓ Completion script written to: {}", path.display());
            println!("  Fish auto-loads completions from this directory — no further setup needed.");
        }
        Shell::Elvish => {
            let path = home.join(".config/elvish/completions/git-id.elv");
            write_completion(shell, &mut cmd, &path);
            println!("✓ Completion script written to: {}", path.display());
        }
        Shell::PowerShell => {
            let path = home.join("Documents/PowerShell/Scripts/git-id.ps1");
            write_completion(shell, &mut cmd, &path);
            println!("✓ Completion script written to: {}", path.display());
            println!("  Make sure your PowerShell profile sources scripts in that directory.");
        }
        _ => {
            generate(shell, &mut cmd, "git-id", &mut std::io::stdout());
        }
    }
}

fn setup_zsh(cmd: &mut clap::Command, home: &std::path::Path) {
    // Prefer oh-my-zsh custom completions dir (auto-loaded, no .zshrc edit needed)
    let omz_dir = home.join(".oh-my-zsh");
    let path = if omz_dir.exists() {
        let p = omz_dir.join("custom/completions/_git-id");
        write_completion(Shell::Zsh, cmd, &p);
        println!("✓ Completion script written to: {}", p.display());
        println!("  Detected oh-my-zsh — completions will load automatically.");
        return;
    } else {
        home.join(".zfunc/_git-id")
    };

    write_completion(Shell::Zsh, cmd, &path);
    println!("✓ Completion script written to: {}", path.display());

    // Auto-append fpath + compinit to .zshrc if not already present
    let zshrc = home.join(".zshrc");
    let zshrc_content = fs::read_to_string(&zshrc).unwrap_or_default();

    let fpath_line = "fpath=(~/.zfunc $fpath)";
    let compinit_line = "autoload -Uz compinit && compinit";

    if !zshrc_content.contains(fpath_line) || !zshrc_content.contains(compinit_line) {
        let mut file = fs::OpenOptions::new()
            .append(true)
            .open(&zshrc)
            .unwrap_or_else(|e| {
                eprintln!("Error opening {}: {}", zshrc.display(), e);
                std::process::exit(1);
            });

        writeln!(file, "\n# git-id shell completions").unwrap();
        if !zshrc_content.contains(fpath_line) {
            writeln!(file, "{}", fpath_line).unwrap();
        }
        if !zshrc_content.contains(compinit_line) {
            writeln!(file, "{}", compinit_line).unwrap();
        }
        println!("✓ Added fpath and compinit to ~/.zshrc");
    }

    println!("  Restart your shell or run: source ~/.zshrc");
}

fn setup_bash(cmd: &mut clap::Command, home: &std::path::Path) {
    // XDG user completions dir — auto-loaded by bash-completion >= 2.2
    let path = home.join(".local/share/bash-completion/completions/git-id");
    write_completion(Shell::Bash, cmd, &path);
    println!("✓ Completion script written to: {}", path.display());

    // If bash-completion 2.x is present, the XDG dir is auto-loaded — no .bashrc edit needed.
    // For older setups, check if ~/.bashrc already sources bash-completion.
    let bashrc = home.join(".bashrc");
    let bashrc_content = fs::read_to_string(&bashrc).unwrap_or_default();

    // Only append a source line if neither bash-completion nor the completions dir is already wired up
    let already_setup = bashrc_content.contains("bash_completion")
        || bashrc_content.contains("bash-completion");

    if !already_setup {
        let source_line = format!(
            "[ -f {} ] && source {}",
            path.display(),
            path.display()
        );
        if !bashrc_content.contains(source_line.as_str()) {
            let mut file = fs::OpenOptions::new()
                .append(true)
                .open(&bashrc)
                .unwrap_or_else(|e| {
                    eprintln!("Error opening {}: {}", bashrc.display(), e);
                    std::process::exit(1);
                });
            writeln!(file, "\n# git-id shell completions\n{}", source_line).unwrap();
            println!("✓ Added source line to ~/.bashrc");
        }
    } else {
        println!("  bash-completion detected — completions will load automatically.");
    }

    println!("  Restart your shell or run: source ~/.bashrc");
}

fn write_completion(shell: Shell, cmd: &mut clap::Command, path: &PathBuf) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap_or_else(|e| {
            eprintln!("Error creating directory {}: {}", parent.display(), e);
            std::process::exit(1);
        });
    }
    let mut file = fs::File::create(path).unwrap_or_else(|e| {
        eprintln!("Error creating file {}: {}", path.display(), e);
        std::process::exit(1);
    });
    generate(shell, cmd, "git-id", &mut file);
    file.flush().unwrap_or_default();
}
