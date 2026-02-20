use crate::config::{ensure_accounts_file, load_accounts, save_accounts};
use crate::models::Account;
use crate::ssh::{add_key_to_agent, fix_key_permissions, gen_ssh_key, ssh_dir, update_ssh_config};
use crate::ui::{color, die, print_hdr, print_info, print_ok, print_warn};
use dialoguer::{Input, Select};
use std::path::PathBuf;

pub fn cmd_add(dry_run: bool) {
    ensure_accounts_file();
    let mut accounts = load_accounts();

    print_hdr("Add a new GitHub account");
    println!();

    let username: String = Input::new()
        .with_prompt(format!("  {}", color("cyan", "GitHub username")))
        .interact_text()
        .unwrap_or_else(|_| die("\nAborted.", 2));

    let host: String = Input::new()
        .with_prompt(format!("  {}", color("cyan", "Host")))
        .default("github.com".to_string())
        .interact_text()
        .unwrap_or_else(|_| die("\nAborted.", 2));

    if accounts.iter().any(|a| a.username == username && a.host == host) {
        die(
            &format!(
                "Account '{}@{}' already exists. Remove it first with: git-id remove {}@{}",
                username, host, username, host
            ),
            2,
        );
    }

    let email: String = Input::new()
        .with_prompt(format!("  {}", color("cyan", "Commit email")))
        .interact_text()
        .unwrap_or_else(|_| die("\nAborted.", 2));

    let remote_choices = &[
        "ssh - use SSH keys (recommended)",
        "https - use personal access token",
        "both - configure SSH and HTTPS",
    ];
    let remote_idx = Select::new()
        .with_prompt(format!("\n  {}", color("cyan", "Remote type")))
        .items(remote_choices)
        .default(0)
        .interact()
        .unwrap_or_else(|_| die("\nAborted.", 2));
    let remote_choice = remote_choices[remote_idx];
    let use_ssh = remote_choice.contains("ssh") || remote_choice.contains("both");
    let use_https = remote_choice.contains("https") || remote_choice.contains("both");

    let mut ssh_key_path = String::new();
    if use_ssh {
        ssh_key_path = setup_ssh_key(&username, &email, dry_run);
    }

    let mut https_token = String::new();
    if use_https {
        print_hdr("HTTPS Token");
        https_token = Input::new()
            .with_prompt(format!(
                "  {}",
                color("cyan", "GitHub personal access token (PAT) (optional)")
            ))
            .allow_empty(true)
            .interact_text()
            .unwrap_or_default();
    }

    let acc = Account {
        username: username.clone(),
        email,
        host: host.clone(),
        ssh_key: ssh_key_path.clone(),
        https_token,
    };
    accounts.push(acc);
    save_accounts(&accounts, dry_run);

    if !ssh_key_path.is_empty() {
        update_ssh_config(&accounts, dry_run);
    }

    println!();
    print_ok(&format!("Account '{}@{}' added!", username, host));
    print_info(&format!(
        "Next: git-id use {}   (inside a repo)  or  git-id use {} --global",
        username, username
    ));
}

/// Interactive prompt to set up (generate or pick) an SSH key.
/// Returns the path to the chosen private key, or empty string on failure.
fn setup_ssh_key(username: &str, email: &str, dry_run: bool) -> String {
    print_hdr("SSH Key");
    let key_choices = vec![
        format!("Generate new ed25519 key  (~/.ssh/id_ed25519_{username})"),
        "Pick from existing ~/.ssh/*.pub keys".to_string(),
    ];
    let key_idx = Select::new()
        .with_prompt(format!("  {}", color("cyan", "SSH key setup")))
        .items(&key_choices)
        .default(0)
        .interact()
        .unwrap_or_else(|_| die("\nAborted.", 2));

    if key_idx == 0 {
        let new_key = gen_ssh_key(username, email, dry_run);
        let ssh_key_path = new_key.to_string_lossy().to_string();
        let pub_key = new_key.with_extension("pub");
        if pub_key.exists() && !dry_run {
            print_hdr("Public key - paste this into GitHub -> Settings -> SSH keys:");
            println!(
                "\n{}\n",
                std::fs::read_to_string(&pub_key).unwrap_or_default().trim()
            );
        }
        ssh_key_path
    } else {
        pick_existing_ssh_key(username, email, dry_run)
    }
}

/// Let the user pick an existing `~/.ssh/*.pub` key.
fn pick_existing_ssh_key(username: &str, email: &str, dry_run: bool) -> String {
    let pub_files: Vec<PathBuf> = {
        let mut v: Vec<PathBuf> = std::fs::read_dir(ssh_dir())
            .map(|rd| {
                rd.filter_map(|e| e.ok())
                    .map(|e| e.path())
                    .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("pub"))
                    .collect()
            })
            .unwrap_or_default();
        v.sort();
        v
    };

    if pub_files.is_empty() {
        print_warn("No .pub files found in ~/.ssh/ - generating a new key instead");
        let new_key = gen_ssh_key(username, email, dry_run);
        return new_key.to_string_lossy().to_string();
    }

    let items: Vec<String> = pub_files
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect();
    let idx = Select::new()
        .with_prompt(format!("  {}", color("cyan", "Pick public key")))
        .items(&items)
        .default(0)
        .interact()
        .unwrap_or_else(|_| die("\nAborted.", 2));

    let chosen_pub = &pub_files[idx];
    let priv_key = chosen_pub.with_extension("");

    if !priv_key.exists() {
        print_warn(&format!("Private key not found: {}", priv_key.display()));
        let yn: String = Input::new()
            .with_prompt("  Generate a new ed25519 key instead? [y/N]")
            .default("N".to_string())
            .interact_text()
            .unwrap_or_default();
        if yn.to_lowercase() == "y" {
            let new_key = gen_ssh_key(username, email, dry_run);
            new_key.to_string_lossy().to_string()
        } else {
            die("Cannot proceed without a valid private key.", 2);
        }
    } else {
        fix_key_permissions(&priv_key);
        add_key_to_agent(&priv_key, dry_run);
        priv_key.to_string_lossy().to_string()
    }
}
