use crate::models::{Account, AccountsFile};
use crate::ui::{backup, die, print_info, print_ok};
use std::path::PathBuf;

pub fn dirs_home() -> PathBuf {
    std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/tmp"))
}

pub fn config_dir() -> PathBuf {
    dirs_home().join(".config").join("git-id")
}

pub fn accounts_file() -> PathBuf {
    config_dir().join("accounts.toml")
}

const EXAMPLE_TOML: &str =
    "# git-id accounts - managed by git-id (safe to edit manually)\n\
     # Add one [[accounts]] section per GitHub identity.\n";

fn accounts_to_toml(accounts: &[Account]) -> String {
    let fields = ["username", "email", "host", "ssh_key", "https_token"];
    let mut lines = vec![
        "# git-id accounts - managed by git-id (safe to edit manually)".to_string(),
        "# Add a new [[accounts]] section to register another identity.".to_string(),
        "".to_string(),
    ];
    for acc in accounts {
        lines.push("[[accounts]]".to_string());
        for &field in &fields {
            let val = match field {
                "username" => &acc.username,
                "email" => &acc.email,
                "host" => &acc.host,
                "ssh_key" => &acc.ssh_key,
                "https_token" => &acc.https_token,
                _ => "",
            };
            let escaped = val.replace('\\', "\\\\").replace('"', "\\\"");
            lines.push(format!("{field} = \"{escaped}\""));
        }
        lines.push("".to_string());
    }
    lines.join("\n") + "\n"
}

pub fn load_accounts() -> Vec<Account> {
    let path = accounts_file();
    if !path.exists() {
        return vec![];
    }
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) => die(&format!("Failed to read {}: {e}", path.display()), 1),
    };
    match toml::from_str::<AccountsFile>(&content) {
        Ok(f) => f.accounts,
        Err(e) => die(&format!("Failed to parse {}: {e}", path.display()), 1),
    }
}

pub fn save_accounts(accounts: &[Account], dry_run: bool) {
    let content = accounts_to_toml(accounts);
    if dry_run {
        print_info("[dry-run] Would write accounts.toml:");
        print!("{content}");
        return;
    }
    let dir = config_dir();
    std::fs::create_dir_all(&dir)
        .unwrap_or_else(|e| die(&format!("Cannot create config dir: {e}"), 1));
    backup(&accounts_file());
    std::fs::write(accounts_file(), &content)
        .unwrap_or_else(|e| die(&format!("Failed to write accounts.toml: {e}"), 1));
    print_ok(&format!("Saved {}", accounts_file().display()));
}

pub fn ensure_accounts_file() {
    if !accounts_file().exists() {
        let dir = config_dir();
        std::fs::create_dir_all(&dir)
            .unwrap_or_else(|e| die(&format!("Cannot create config dir: {e}"), 1));
        std::fs::write(accounts_file(), EXAMPLE_TOML)
            .unwrap_or_else(|e| die(&format!("Failed to create accounts.toml: {e}"), 1));
        print_info(&format!(
            "Created {} (no accounts yet - run 'git-id add')",
            accounts_file().display()
        ));
    }
}

pub fn account_id(acc: &Account) -> String {
    let host = if acc.host.is_empty() { "github.com" } else { &acc.host };
    format!("{}@{}", acc.username, host)
}

pub fn ssh_host_alias(acc: &Account) -> String {
    let host = if acc.host.is_empty() { "github.com" } else { &acc.host };
    format!("{host}-{}", acc.username)
}

pub fn find_account(key: &str) -> Option<Account> {
    let accounts = load_accounts();
    if let Some((uname, host)) = key.split_once('@') {
        return accounts
            .into_iter()
            .find(|a| a.username == uname && a.host == host);
    }
    let matches: Vec<Account> = accounts.into_iter().filter(|a| a.username == key).collect();
    match matches.len() {
        1 => Some(matches.into_iter().next().unwrap()),
        0 => None,
        _ => {
            let hints: Vec<String> = matches
                .iter()
                .map(|a| {
                    let host = if a.host.is_empty() { "github.com" } else { &a.host };
                    format!("'{key}@{host}'")
                })
                .collect();
            die(
                &format!(
                    "Multiple accounts with username '{key}'.\n  Specify host to disambiguate: {}",
                    hints.join("  or  ")
                ),
                2,
            )
        }
    }
}
