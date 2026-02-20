use crate::config::{account_id, find_account, load_accounts, save_accounts};
use crate::ssh::{
    add_key_to_agent, fix_key_permissions, gen_ssh_key, make_stanza, ssh_dir, update_ssh_config,
};
use crate::ui::{color, die, print_hdr, print_info, print_ok, print_warn};
use dialoguer::{Input, Select};
use std::path::PathBuf;

pub fn cmd_ssh_gen(username: &str, dry_run: bool) {
    let acc = find_account(username)
        .unwrap_or_else(|| die(&format!("Account '{username}' not found."), 2));

    let key = gen_ssh_key(&acc.username, &acc.email, dry_run);
    fix_key_permissions(&key);

    let mut accounts = load_accounts();
    let uid = account_id(&acc);
    for a in accounts.iter_mut() {
        if account_id(a) == uid {
            a.ssh_key = key.to_string_lossy().to_string();
        }
    }
    save_accounts(&accounts, dry_run);
    update_ssh_config(&accounts, dry_run);

    let pub_key = key.with_extension("pub");
    if pub_key.exists() && !dry_run {
        print_hdr("Public key - paste into GitHub -> Settings -> SSH keys:");
        println!("\n{}\n", std::fs::read_to_string(&pub_key).unwrap_or_default().trim());
    }
}

pub fn cmd_ssh_pick(username: &str, dry_run: bool) {
    let acc = find_account(username)
        .unwrap_or_else(|| die(&format!("Account '{username}' not found."), 2));

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
        die("No .pub files found in ~/.ssh/", 1);
    }

    print_hdr(&format!("Pick SSH key for '{username}'"));
    let items: Vec<String> = pub_files
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect();
    let idx = Select::new()
        .with_prompt(format!("  {}", color("cyan", "Select public key")))
        .items(&items)
        .default(0)
        .interact()
        .unwrap_or_else(|_| die("\nAborted.", 2));

    let chosen_pub = &pub_files[idx];
    let priv_key = chosen_pub.with_extension("");

    let final_priv = if !priv_key.exists() {
        print_warn(&format!("Private key not found: {}", priv_key.display()));
        let yn: String = Input::new()
            .with_prompt("  Generate a new ed25519 key instead? [y/N]")
            .default("N".to_string())
            .interact_text()
            .unwrap_or_default();
        if yn.to_lowercase() == "y" {
            gen_ssh_key(&acc.username, &acc.email, dry_run)
        } else {
            die("Cannot proceed without a private key.", 2);
        }
    } else {
        fix_key_permissions(&priv_key);
        add_key_to_agent(&priv_key, dry_run);
        priv_key.clone()
    };

    let mut accounts = load_accounts();
    let uid = account_id(&acc);
    for a in accounts.iter_mut() {
        if account_id(a) == uid {
            a.ssh_key = final_priv.to_string_lossy().to_string();
        }
    }
    save_accounts(&accounts, dry_run);
    update_ssh_config(&accounts, dry_run);
    print_ok(&format!("SSH key for '{username}' -> {}", final_priv.display()));
}

pub fn cmd_ssh_config(dry_run: bool) {
    let accounts = load_accounts();
    if accounts.is_empty() {
        print_info("No accounts configured. Run: git-id add");
        return;
    }
    update_ssh_config(&accounts, dry_run);
    print_hdr("Generated SSH config stanzas:");
    for acc in &accounts {
        println!("{}", make_stanza(acc));
    }
}
