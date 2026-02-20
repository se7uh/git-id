use crate::cli::build_command;
use clap_complete::{generate, Shell};
use std::fs;
use std::io::Write;
use std::path::PathBuf;

pub fn cmd_completions(shell: Shell) {
    let home = dirs::home_dir().expect("Could not determine home directory");

    match shell {
        Shell::Zsh => setup_zsh(&home),
        Shell::Bash => setup_bash(&home),
        Shell::Fish => {
            let path = home.join(".config/fish/completions/git-id.fish");
            write_completion(shell, &path);
            println!("✓ Completion script written to: {}", path.display());
            println!("  Fish auto-loads completions from this directory — no further setup needed.");
        }
        Shell::Elvish => {
            let path = home.join(".config/elvish/completions/git-id.elv");
            write_completion(shell, &path);
            println!("✓ Completion script written to: {}", path.display());
        }
        Shell::PowerShell => {
            let path = home.join("Documents/PowerShell/Scripts/git-id.ps1");
            write_completion(shell, &path);
            println!("✓ Completion script written to: {}", path.display());
            println!("  Make sure your PowerShell profile sources scripts in that directory.");
        }
        _ => {
            generate(shell, &mut build_command(), "git-id", &mut std::io::stdout());
        }
    }
}

fn setup_zsh(home: &std::path::Path) {
    let omz_dir = home.join(".oh-my-zsh");
    if omz_dir.exists() {
        let p = omz_dir.join("custom/completions/_git-id");
        write_completion_zsh(&p);
        println!("✓ Completion script written to: {}", p.display());
        println!("  Detected oh-my-zsh — completions will load automatically.");
        return;
    }

    let path = home.join(".zfunc/_git-id");
    write_completion_zsh(&path);
    println!("✓ Completion script written to: {}", path.display());

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

fn setup_bash(home: &std::path::Path) {
    let path = home.join(".local/share/bash-completion/completions/git-id");
    write_completion(Shell::Bash, &path);
    println!("✓ Completion script written to: {}", path.display());

    let bashrc = home.join(".bashrc");
    let bashrc_content = fs::read_to_string(&bashrc).unwrap_or_default();
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

/// Generate zsh script, inject account completion function, and write to path.
fn write_completion_zsh(path: &PathBuf) {
    let mut buf: Vec<u8> = Vec::new();
    generate(Shell::Zsh, &mut build_command(), "git-id", &mut buf);

    // Replace _default completer on username args with our custom account completer.
    let script = String::from_utf8_lossy(&buf).replace(
        "':username -- GitHub username (or username@host):_default'",
        "':username -- GitHub username (or username@host):_git_id_accounts'",
    );

    // Append the account completion helper that reads accounts.toml directly.
    let helper = r#"
_git_id_accounts() {
  local accounts_file="${XDG_CONFIG_HOME:-$HOME/.config}/git-id/accounts.toml"
  [[ -f "$accounts_file" ]] || return
  local -a candidates
  local u h
  while IFS= read -r line; do
    if [[ "$line" =~ '^username = "(.+)"' ]]; then
      u="${match[1]}"
    elif [[ "$line" =~ '^host = "(.+)"' ]]; then
      h="${match[1]}"
    fi
    if [[ -n "$u" && -n "$h" ]]; then
      candidates+=("${u}@${h}")
      u=""
      h=""
    fi
  done < "$accounts_file"
  _describe 'account' candidates
}
"#;

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
    file.write_all(script.as_bytes()).unwrap();
    file.write_all(helper.as_bytes()).unwrap();
    file.flush().unwrap_or_default();
}

fn write_completion(shell: Shell, path: &PathBuf) {
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
    generate(shell, &mut build_command(), "git-id", &mut file);
    file.flush().unwrap_or_default();
}
