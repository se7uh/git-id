use crate::config::{account_id, find_account, load_accounts, save_accounts};
use crate::ssh::{remove_stanza, ssh_config_path, MARKER_E, MARKER_S};
use crate::ui::{backup, color, die, print_info, print_ok};
use dialoguer::Input;
use std::path::{Path, PathBuf};

pub fn cmd_remove(username: &str, yes: bool, delete_keys: bool, dry_run: bool) {
    let acc = find_account(username)
        .unwrap_or_else(|| die(&format!("Account '{username}' not found. Run: git-id list"), 2));

    if !yes {
        let host = if acc.host.is_empty() { "github.com" } else { &acc.host };
        println!(
            "\n  {} {}  {}",
            color("yellow", "About to remove account:"),
            color("bold", &acc.username),
            color("dim", host)
        );
        println!("    email: {}", acc.email);
        if !acc.ssh_key.is_empty() {
            println!("    key  : {}", acc.ssh_key);
        }
        let ans: String = Input::new()
            .with_prompt("\n  Confirm removal? [y/N]")
            .default("N".to_string())
            .interact_text()
            .unwrap_or_default();
        if ans.to_lowercase() != "y" {
            print_info("Aborted.");
            return;
        }
    }

    remove_ssh_config_stanza(&account_id(&acc), dry_run);

    let uid = account_id(&acc);
    let accounts = load_accounts();
    let new_accounts: Vec<_> = accounts.into_iter().filter(|a| account_id(a) != uid).collect();
    save_accounts(&new_accounts, dry_run);

    if !acc.ssh_key.is_empty() {
        handle_key_files(&acc.ssh_key, delete_keys, dry_run);
    }

    if !dry_run {
        print_ok(&format!("Account '{}' removed.", account_id(&acc)));
    }
}

fn remove_ssh_config_stanza(acct_id: &str, dry_run: bool) {
    let cfg = ssh_config_path();
    if !cfg.exists() {
        return;
    }
    let content = std::fs::read_to_string(&cfg).unwrap_or_default();
    let start = MARKER_S.replace("{id}", acct_id);
    let end_marker = MARKER_E.replace("{id}", acct_id);
    if !content.contains(&start) {
        print_info(&format!("No SSH config stanza found for '{acct_id}' - skipping"));
        return;
    }
    let new_content = remove_stanza(&content, &start, &end_marker);
    if dry_run {
        print_info(&format!("[dry-run] Would remove SSH config stanza for '{acct_id}'"));
    } else {
        backup(&cfg);
        std::fs::write(&cfg, &new_content)
            .unwrap_or_else(|e| crate::ui::die(&format!("Failed to write SSH config: {e}"), 1));
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&cfg, std::fs::Permissions::from_mode(0o600));
        print_ok(&format!("Removed SSH config stanza for '{acct_id}'"));
    }
}

fn handle_key_files(ssh_key: &str, delete_keys: bool, dry_run: bool) {
    let priv_key = PathBuf::from(ssh_key);
    let pub_key = priv_key.with_extension("pub");
    if delete_keys {
        for f in [&priv_key, &pub_key] {
            if f.exists() {
                if dry_run {
                    print_info(&format!("[dry-run] Would delete {}", f.display()));
                } else {
                    let _ = std::fs::remove_file(f);
                    print_ok(&format!("Deleted {}", f.display()));
                }
            }
        }
    } else {
        let existing: Vec<&Path> = [priv_key.as_path(), pub_key.as_path()]
            .iter()
            .copied()
            .filter(|f| f.exists())
            .collect();
        if !existing.is_empty() {
            print_info("SSH key files kept (use --delete-keys to also remove them):");
            for f in existing {
                println!("    {}", color("dim", &f.to_string_lossy()));
            }
        }
    }
}
