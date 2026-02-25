use crate::config::find_account;
use crate::git::{
    build_https_url, build_ssh_url, get_remote_url, in_git_repo, list_remotes, parse_remote_url,
    set_git_config, set_remote_url,
};
use crate::models::Account;
use crate::ui::{die, print_info, print_ok, print_warn};

pub fn cmd_use(username: &str, global: bool, force_ssh: bool, force_https: bool, dry_run: bool) {
    let acc = find_account(username)
        .unwrap_or_else(|| die(&format!("Account '{username}' not found. Run: git-id list"), 2));

    let scope = if global { "global" } else { "local" };
    if scope == "local" && !in_git_repo() {
        die("Not inside a git repository. Use --global or cd into a repo.", 2);
    }

    set_git_config("user.name", &acc.username, scope, dry_run);
    set_git_config("user.email", &acc.email, scope, dry_run);
    print_ok(&format!("Git identity ({scope}): {} <{}>", acc.username, acc.email));

    if scope == "local" {
        update_matching_remotes(&acc, force_ssh, force_https, dry_run);
    }
}

fn update_matching_remotes(acc: &Account, force_ssh: bool, force_https: bool, dry_run: bool) {
    let token = &acc.https_token;
    let ssh_key = &acc.ssh_key;
    let remotes = list_remotes();

    if remotes.is_empty() {
        print_info("No remotes found - skipping remote URL update (identity set)");
        return;
    }

    if force_ssh && force_https {
        die("Cannot use --ssh and --https together.", 2);
    }

    let account_host = if acc.host.is_empty() {
        "github.com"
    } else {
        &acc.host
    };
    let account_alias_prefix = format!("git@{}:", crate::config::ssh_host_alias(acc));

    for remote in remotes {
        let remote_url = get_remote_url(&remote);
        if remote_url.is_empty() {
            continue;
        }
        let parsed = match parse_remote_url(&remote_url) {
            Some(p) => p,
            None => {
                print_warn(&format!(
                    "Unrecognised remote URL format for '{remote}': {remote_url:?} - skipping"
                ));
                continue;
            }
        };
        let (current_fmt, host, owner, repo) = parsed;
        let is_origin = remote == "origin";
        let matches_identity_remote = host == account_host && owner == acc.username;
        let already_on_identity_alias = remote_url.starts_with(&account_alias_prefix);
        if !is_origin && !matches_identity_remote && !already_on_identity_alias {
            continue;
        }

        let mut target_fmt = if force_ssh {
            "ssh".to_string()
        } else if force_https {
            "https".to_string()
        } else {
            current_fmt
        };

        if target_fmt == "ssh" {
            if ssh_key.is_empty() {
                print_warn("No SSH key configured for this account; falling back to HTTPS");
                target_fmt = "https".to_string();
            } else {
                let new_url = build_ssh_url(acc, &owner, &repo);
                set_remote_url(&remote, &new_url, dry_run);
                continue;
            }
        }
        if target_fmt == "https" {
            let new_url = build_https_url(token, &host, &owner, &repo);
            set_remote_url(&remote, &new_url, dry_run);
        }
    }
}
