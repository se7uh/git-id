use crate::config::{accounts_file, dirs_home, ensure_accounts_file, load_accounts, ssh_host_alias};
use crate::git::{get_git_config, in_git_repo};
use crate::ui::{color, print_hdr, print_info};
use std::path::PathBuf;

pub fn cmd_list() {
    ensure_accounts_file();
    let accounts = load_accounts();

    if accounts.is_empty() {
        print_info("No accounts configured yet. Run: git-id add");
        print_info(&format!("Config file: {}", accounts_file().display()));
        return;
    }

    let in_repo = in_git_repo();
    let local_email = if in_repo {
        get_git_config("user.email", "local")
    } else {
        String::new()
    };
    let global_email = get_git_config("user.email", "global");

    print_hdr(&format!("Configured accounts  ({} total)", accounts.len()));

    for acc in &accounts {
        let username = &acc.username;
        let email = &acc.email;
        let host = if acc.host.is_empty() { "github.com" } else { &acc.host };
        let ssh_key = PathBuf::from(acc.ssh_key.replace('~', &dirs_home().to_string_lossy()));
        let token = &acc.https_token;

        let priv_ok = if !acc.ssh_key.is_empty() && ssh_key.exists() {
            color("green", "yes")
        } else {
            color("red", "no")
        };
        let pub_ok = if !acc.ssh_key.is_empty() && ssh_key.with_extension("pub").exists() {
            color("green", "yes")
        } else {
            color("red", "no")
        };
        let tok_ok = if !token.is_empty() {
            color("green", "yes")
        } else {
            color("dim", "-")
        };

        let mut tags = String::new();
        if !email.is_empty() && *email == local_email {
            tags.push_str(&format!("  {}", color("green", "[active:local]")));
        }
        if !email.is_empty() && *email == global_email {
            tags.push_str(&format!("  {}", color("yellow", "[active:global]")));
        }

        let ssh_display = if acc.ssh_key.is_empty() {
            color("dim", "(none)")
        } else {
            acc.ssh_key.clone()
        };
        let alias = ssh_host_alias(acc);

        println!(
            "\n  {}  {}{}\n    email  : {}\n    ssh    : {}  priv:{}  pub:{}\n    token  : {}\n    alias  : {}",
            color("bold", username),
            color("dim", host),
            tags,
            email,
            ssh_display,
            priv_ok,
            pub_ok,
            tok_ok,
            alias
        );
    }
    println!();
}
