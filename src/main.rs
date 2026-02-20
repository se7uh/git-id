use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::{generate, Shell};
use dialoguer::{Input, Select};
use serde::{Deserialize, Serialize};
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

// CLI definitions

#[derive(Parser)]
#[command(
    name = "git-id",
    version = "1.0.0",
    about = "Manage multiple GitHub accounts on one machine."
)]
struct Cli {
    /// Preview changes without modifying any files
    #[arg(long, global = true)]
    dry_run: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Add a new account (interactive wizard)
    Add,
    /// List all accounts with status
    List,
    /// Set identity for repo or globally
    Use {
        /// GitHub username (or username@host)
        username: String,
        /// Apply to global git config instead of current repo
        #[arg(long = "global")]
        global: bool,
        /// Convert remote URL to SSH format
        #[arg(long = "ssh")]
        force_ssh: bool,
        /// Convert remote URL to HTTPS format
        #[arg(long = "https")]
        force_https: bool,
    },
    /// Remove an account and its SSH config stanza
    Remove {
        /// GitHub username (or username@host)
        username: String,
        /// Skip confirmation prompt
        #[arg(long, short = 'y')]
        yes: bool,
        /// Also delete the SSH private and public key files
        #[arg(long)]
        delete_keys: bool,
    },
    /// SSH key management subcommands
    Ssh {
        #[command(subcommand)]
        subcommand: SshCommands,
    },
    /// Show current identity and loaded SSH keys
    Status,
    /// Generate shell completion script
    Completions {
        /// Shell to generate completions for
        shell: Shell,
    },
}

#[derive(Subcommand)]
enum SshCommands {
    /// Generate a new ed25519 key
    Gen {
        /// GitHub username (or username@host)
        username: String,
    },
    /// Pick an existing ~/.ssh/*.pub key
    Pick {
        /// GitHub username (or username@host)
        username: String,
    },
    /// Write ~/.ssh/config stanzas for all accounts
    Config,
}

// Data model

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct Account {
    #[serde(default)]
    username: String,
    #[serde(default)]
    email: String,
    #[serde(default)]
    host: String,
    #[serde(default)]
    ssh_key: String,
    #[serde(default)]
    https_token: String,
}

#[derive(Debug, Deserialize)]
struct AccountsFile {
    #[serde(default)]
    accounts: Vec<Account>,
}

// Paths

fn config_dir() -> PathBuf {
    dirs_home().join(".config").join("git-id")
}

fn accounts_file() -> PathBuf {
    config_dir().join("accounts.toml")
}

fn ssh_dir() -> PathBuf {
    dirs_home().join(".ssh")
}

fn ssh_config_path() -> PathBuf {
    dirs_home().join(".ssh").join("config")
}

fn dirs_home() -> PathBuf {
    std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/tmp"))
}

// ANSI colour helpers

fn is_tty() -> bool {
    use std::io::IsTerminal;
    std::io::stdout().is_terminal()
}

fn color(code: &str, text: &str) -> String {
    if !is_tty() {
        return text.to_string();
    }
    let code_str = match code {
        "bold" => "1",
        "dim" => "2",
        "red" => "31",
        "green" => "32",
        "yellow" => "33",
        "blue" => "34",
        "magenta" => "35",
        "cyan" => "36",
        _ => "0",
    };
    format!("\x1b[{code_str}m{text}\x1b[0m")
}

fn print_ok(msg: &str) {
    println!("{} {}", color("green", "OK"), msg);
}

fn print_warn(msg: &str) {
    eprintln!("{} {}", color("yellow", "!"), msg);
}

fn print_err(msg: &str) {
    eprintln!("{} {}", color("red", "ERR"), msg);
}

fn print_info(msg: &str) {
    println!("{} {}", color("cyan", "->"), msg);
}

fn print_hdr(msg: &str) {
    println!("\n{}", color("bold", msg));
}

fn die(msg: &str, code: i32) -> ! {
    print_err(msg);
    std::process::exit(code);
}

// Backup helper

fn backup(path: &Path) -> Option<PathBuf> {
    if !path.exists() {
        return None;
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let dst = path
        .parent()
        .unwrap_or(Path::new("."))
        .join(format!("{}.bak.{}", path.file_name().unwrap().to_string_lossy(), now));
    if std::fs::copy(path, &dst).is_ok() {
        print_info(&format!(
            "Backed up {} -> {}",
            path.file_name().unwrap().to_string_lossy(),
            dst.file_name().unwrap().to_string_lossy()
        ));
        Some(dst)
    } else {
        None
    }
}

// TOML helpers

const EXAMPLE_TOML: &str = "# git-id accounts - managed by git-id (safe to edit manually)\n# Add one [[accounts]] section per GitHub identity.\n";

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

fn load_accounts() -> Vec<Account> {
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

fn save_accounts(accounts: &[Account], dry_run: bool) {
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

fn ensure_accounts_file() {
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

// Account helpers

fn account_id(acc: &Account) -> String {
    let host = if acc.host.is_empty() { "github.com" } else { &acc.host };
    format!("{}@{}", acc.username, host)
}

fn ssh_host_alias(acc: &Account) -> String {
    let host = if acc.host.is_empty() { "github.com" } else { &acc.host };
    format!("{host}-{}", acc.username)
}

fn default_key_path(username: &str) -> PathBuf {
    ssh_dir().join(format!("id_ed25519_{username}"))
}

fn find_account(key: &str) -> Option<Account> {
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

// SSH helpers

fn gen_ssh_key(username: &str, email: &str, dry_run: bool) -> PathBuf {
    let key = default_key_path(username);
    if key.exists() {
        print_warn(&format!(
            "Key {} already exists - skipping (delete it first to regenerate)",
            key.display()
        ));
        return key;
    }
    let ssh = ssh_dir();
    if !ssh.exists() {
        use std::os::unix::fs::DirBuilderExt;
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&ssh)
            .unwrap_or_else(|e| die(&format!("Cannot create ~/.ssh: {e}"), 1));
    }
    let key_str = key.to_string_lossy().to_string();
    let cmd_args = [
        "ssh-keygen", "-t", "ed25519", "-C", email, "-f", &key_str, "-N", "",
    ];
    if dry_run {
        print_info(&format!("[dry-run] Would run: {}", cmd_args.join(" ")));
        return key;
    }
    let result = Command::new(cmd_args[0])
        .args(&cmd_args[1..])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output();
    match result {
        Ok(out) if out.status.success() => {}
        Ok(out) => die(
            &format!(
                "ssh-keygen failed: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            ),
            1,
        ),
        Err(e) => die(&format!("Failed to run ssh-keygen: {e}"), 1),
    }
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(&key, std::fs::Permissions::from_mode(0o600));
    let pub_key = key.with_extension("pub");
    if pub_key.exists() {
        let _ = std::fs::set_permissions(&pub_key, std::fs::Permissions::from_mode(0o644));
    }
    print_ok(&format!("Generated {}", key.display()));
    add_key_to_agent(&key, false);
    key
}

fn add_key_to_agent(key: &Path, dry_run: bool) {
    if !key.exists() {
        print_warn(&format!(
            "Key {} not found - cannot add to ssh-agent",
            key.display()
        ));
        return;
    }
    if dry_run {
        print_info(&format!("[dry-run] Would run: ssh-add {}", key.display()));
        return;
    }
    if std::env::var("SSH_AUTH_SOCK").is_err() {
        print_warn("SSH_AUTH_SOCK not set - ssh-agent may not be running");
    }
    let result = Command::new("ssh-add")
        .arg(key)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output();
    match result {
        Ok(out) if out.status.success() => {
            print_ok(&format!("Added {} to ssh-agent", key.display()))
        }
        Ok(out) => print_warn(&format!(
            "ssh-add failed (is ssh-agent running?): {}",
            String::from_utf8_lossy(&out.stderr).trim()
        )),
        Err(e) => print_warn(&format!("Failed to run ssh-add: {e}")),
    }
}

fn fix_key_permissions(key: &Path) {
    use std::os::unix::fs::PermissionsExt;
    if key.exists() {
        let _ = std::fs::set_permissions(key, std::fs::Permissions::from_mode(0o600));
        print_ok(&format!("chmod 600 {}", key.display()));
    }
    let pub_key = key.with_extension("pub");
    if pub_key.exists() {
        let _ = std::fs::set_permissions(&pub_key, std::fs::Permissions::from_mode(0o644));
        print_ok(&format!("chmod 644 {}", pub_key.display()));
    }
}

// SSH config

const MARKER_S: &str = "# >>> git-id: {id} >>>";
const MARKER_E: &str = "# <<< git-id: {id} <<<";

fn make_stanza(acc: &Account) -> String {
    let acct_id = account_id(acc);
    let alias = ssh_host_alias(acc);
    let host = if acc.host.is_empty() { "github.com" } else { &acc.host };
    let keyfile = if acc.ssh_key.is_empty() {
        format!("~/.ssh/id_ed25519_{}", acc.username)
    } else {
        acc.ssh_key.clone()
    };
    let start = MARKER_S.replace("{id}", &acct_id);
    let end = MARKER_E.replace("{id}", &acct_id);
    format!(
        "{start}\nHost {alias}\n    HostName {host}\n    User git\n    IdentityFile {keyfile}\n    IdentitiesOnly yes\n{end}\n"
    )
}

fn update_ssh_config(accounts: &[Account], dry_run: bool) {
    let ssh = ssh_dir();
    if !ssh.exists() {
        use std::os::unix::fs::DirBuilderExt;
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&ssh)
            .unwrap_or_else(|e| die(&format!("Cannot create ~/.ssh: {e}"), 1));
    }
    let cfg = ssh_config_path();
    let mut existing = if cfg.exists() {
        std::fs::read_to_string(&cfg).unwrap_or_default()
    } else {
        String::new()
    };

    for acc in accounts {
        let acct_id = account_id(acc);
        let stanza = make_stanza(acc);
        let start = MARKER_S.replace("{id}", &acct_id);
        let end = MARKER_E.replace("{id}", &acct_id);
        if existing.contains(&start) {
            existing = replace_stanza(&existing, &start, &end, &stanza);
        } else {
            let trimmed = existing.trim_end_matches('\n');
            existing = format!("{trimmed}\n\n{stanza}");
        }
    }

    if dry_run {
        print_info("[dry-run] Would write ~/.ssh/config:");
        print!("{existing}");
        return;
    }

    backup(&cfg);
    std::fs::write(&cfg, &existing)
        .unwrap_or_else(|e| die(&format!("Failed to write SSH config: {e}"), 1));
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(&cfg, std::fs::Permissions::from_mode(0o600));
    print_ok(&format!("Updated {}", cfg.display()));
}

fn replace_stanza(content: &str, start: &str, end: &str, replacement: &str) -> String {
    let start_pos = match content.find(start) {
        Some(p) => p,
        None => return content.to_string(),
    };
    let end_offset = match content[start_pos..].find(end) {
        Some(p) => p,
        None => return content.to_string(),
    };
    let end_pos = start_pos + end_offset + end.len();
    let end_pos = if content.as_bytes().get(end_pos) == Some(&b'\n') {
        end_pos + 1
    } else {
        end_pos
    };
    format!("{}{}{}", &content[..start_pos], replacement, &content[end_pos..])
}

fn remove_stanza(content: &str, start: &str, end: &str) -> String {
    let start_pos = match content.find(start) {
        Some(p) => p,
        None => return content.to_string(),
    };
    let end_offset = match content[start_pos..].find(end) {
        Some(p) => p,
        None => return content.to_string(),
    };
    let end_pos = start_pos + end_offset + end.len();
    let end_pos = if content.as_bytes().get(end_pos) == Some(&b'\n') {
        end_pos + 1
    } else {
        end_pos
    };
    let start_pos = if start_pos > 0 && content.as_bytes().get(start_pos - 1) == Some(&b'\n') {
        start_pos - 1
    } else {
        start_pos
    };
    format!("{}{}", &content[..start_pos], &content[end_pos..])
}

// Git helpers

fn run_git(args: &[&str]) -> (i32, String, String) {
    let out = Command::new("git")
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output();
    match out {
        Ok(o) => (
            o.status.code().unwrap_or(1),
            String::from_utf8_lossy(&o.stdout).trim().to_string(),
            String::from_utf8_lossy(&o.stderr).trim().to_string(),
        ),
        Err(_) => (1, String::new(), "git not found".to_string()),
    }
}

fn in_git_repo() -> bool {
    run_git(&["rev-parse", "--git-dir"]).0 == 0
}

fn get_git_config(key: &str, scope: &str) -> String {
    let flag = format!("--{scope}");
    let (code, out, _) = run_git(&["config", &flag, key]);
    if code == 0 { out } else { String::new() }
}

fn set_git_config(key: &str, value: &str, scope: &str, dry_run: bool) {
    let flag = format!("--{scope}");
    if dry_run {
        print_info(&format!("[dry-run] git config {flag} {key} {value:?}"));
        return;
    }
    let (code, _, errmsg) = run_git(&["config", &flag, key, value]);
    if code != 0 {
        print_warn(&format!("git config {flag} {key}: {errmsg}"));
    }
}

fn get_remote_url(remote: &str) -> String {
    let (code, url, _) = run_git(&["remote", "get-url", remote]);
    if code == 0 { url } else { String::new() }
}

/// Strips a git-id username suffix from an SSH host alias.
/// e.g. "github.com-alice" → "github.com", "github.com" → "github.com"
/// A suffix is recognised as a username when it contains no dots.
fn strip_host_alias_suffix(raw_host: &str) -> String {
    if let Some(last_dash) = raw_host.rfind('-') {
        let suffix = &raw_host[last_dash + 1..];
        if !suffix.contains('.') {
            return raw_host[..last_dash].to_string();
        }
    }
    raw_host.to_string()
}

fn parse_remote_url(url: &str) -> Option<(String, String, String, String)> {
    if let Some(rest) = url.strip_prefix("git@") {
        if let Some(colon) = rest.find(':') {
            let raw_host = &rest[..colon];
            let path = &rest[colon + 1..];
            let path = path.trim_end_matches(".git");
            if let Some(slash) = path.find('/') {
                let owner = &path[..slash];
                let repo = &path[slash + 1..];
                let host = strip_host_alias_suffix(raw_host);
                return Some(("ssh".to_string(), host, owner.to_string(), repo.to_string()));
            }
        }
    }
    if let Some(rest) = url.strip_prefix("https://") {
        let rest = if let Some(at) = rest.find('@') {
            &rest[at + 1..]
        } else {
            rest
        };
        let rest = rest.trim_end_matches(".git");
        let parts: Vec<&str> = rest.splitn(3, '/').collect();
        if parts.len() == 3 {
            return Some((
                "https".to_string(),
                parts[0].to_string(),
                parts[1].to_string(),
                parts[2].to_string(),
            ));
        }
    }
    None
}

fn build_ssh_url(acc: &Account, owner: &str, repo: &str) -> String {
    let alias = ssh_host_alias(acc);
    format!("git@{alias}:{owner}/{repo}.git")
}

fn build_https_url(token: &str, host: &str, owner: &str, repo: &str) -> String {
    if !token.is_empty() {
        format!("https://{token}@{host}/{owner}/{repo}.git")
    } else {
        format!("https://{host}/{owner}/{repo}.git")
    }
}

fn set_remote_url(remote: &str, url: &str, dry_run: bool) {
    if dry_run {
        print_info(&format!("[dry-run] git remote set-url {remote} {url}"));
        return;
    }
    let (code, _, errmsg) = run_git(&["remote", "set-url", remote, url]);
    if code != 0 {
        print_warn(&format!("Could not set remote URL: {errmsg}"));
    } else {
        print_ok(&format!("Remote '{remote}' -> {url}"));
    }
}

fn repo_name() -> String {
    let (_, out, _) = run_git(&["rev-parse", "--show-toplevel"]);
    if out.is_empty() {
        ".".to_string()
    } else {
        PathBuf::from(&out)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| ".".to_string())
    }
}

// Command: add

fn cmd_add(dry_run: bool) {
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
            let new_key = gen_ssh_key(&username, &email, dry_run);
            ssh_key_path = new_key.to_string_lossy().to_string();
            let pub_key = new_key.with_extension("pub");
            if pub_key.exists() && !dry_run {
                print_hdr("Public key - paste this into GitHub -> Settings -> SSH keys:");
                println!(
                    "\n{}\n",
                    std::fs::read_to_string(&pub_key).unwrap_or_default().trim()
                );
            }
        } else {
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
                let new_key = gen_ssh_key(&username, &email, dry_run);
                ssh_key_path = new_key.to_string_lossy().to_string();
            } else {
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
                        let new_key = gen_ssh_key(&username, &email, dry_run);
                        ssh_key_path = new_key.to_string_lossy().to_string();
                    } else {
                        die("Cannot proceed without a valid private key.", 2);
                    }
                } else {
                    ssh_key_path = priv_key.to_string_lossy().to_string();
                    fix_key_permissions(&priv_key);
                    add_key_to_agent(&priv_key, dry_run);
                }
            }
        }
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

// Command: list

fn cmd_list() {
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

// Command: use

fn cmd_use(username: &str, global: bool, force_ssh: bool, force_https: bool, dry_run: bool) {
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
        update_remote_origin(&acc, force_ssh, force_https, dry_run);
    }
}

fn update_remote_origin(acc: &Account, force_ssh: bool, force_https: bool, dry_run: bool) {
    let token = &acc.https_token;
    let ssh_key = &acc.ssh_key;

    let remote_url = get_remote_url("origin");
    if remote_url.is_empty() {
        print_info("No 'origin' remote - skipping remote URL update (identity set)");
        return;
    }

    let parsed = match parse_remote_url(&remote_url) {
        Some(p) => p,
        None => {
            print_warn(&format!("Unrecognised remote URL format: {remote_url:?} - skipping"));
            return;
        }
    };

    let (current_fmt, host, owner, repo) = parsed;

    if force_ssh && force_https {
        die("Cannot use --ssh and --https together.", 2);
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
            set_remote_url("origin", &new_url, dry_run);
            return;
        }
    }
    if target_fmt == "https" {
        let new_url = build_https_url(token, &host, &owner, &repo);
        set_remote_url("origin", &new_url, dry_run);
    }
}

// Command: remove

fn cmd_remove(username: &str, yes: bool, delete_keys: bool, dry_run: bool) {
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

    let cfg = ssh_config_path();
    if cfg.exists() {
        let acct_id = account_id(&acc);
        let content = std::fs::read_to_string(&cfg).unwrap_or_default();
        let start = MARKER_S.replace("{id}", &acct_id);
        let end_marker = MARKER_E.replace("{id}", &acct_id);
        if content.contains(&start) {
            let new_content = remove_stanza(&content, &start, &end_marker);
            if dry_run {
                print_info(&format!("[dry-run] Would remove SSH config stanza for '{acct_id}'"));
            } else {
                backup(&cfg);
                std::fs::write(&cfg, &new_content)
                    .unwrap_or_else(|e| die(&format!("Failed to write SSH config: {e}"), 1));
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(&cfg, std::fs::Permissions::from_mode(0o600));
                print_ok(&format!("Removed SSH config stanza for '{acct_id}'"));
            }
        } else {
            print_info(&format!("No SSH config stanza found for '{acct_id}' - skipping"));
        }
    }

    let uid = account_id(&acc);
    let accounts = load_accounts();
    let new_accounts: Vec<Account> = accounts
        .into_iter()
        .filter(|a| account_id(a) != uid)
        .collect();
    save_accounts(&new_accounts, dry_run);

    if !acc.ssh_key.is_empty() {
        let priv_key = PathBuf::from(&acc.ssh_key);
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

    if !dry_run {
        print_ok(&format!("Account '{}' removed.", account_id(&acc)));
    }
}

// Command: ssh gen

fn cmd_ssh_gen(username: &str, dry_run: bool) {
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

// Command: ssh pick

fn cmd_ssh_pick(username: &str, dry_run: bool) {
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

// Command: ssh config

fn cmd_ssh_config(dry_run: bool) {
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

// Command: status

fn cmd_status() {
    print_hdr("git-id status");

    let g_name = get_git_config("user.name", "global");
    let g_email = get_git_config("user.email", "global");
    println!("\n  {}", color("bold", "Global git identity"));
    println!(
        "    name : {}",
        if g_name.is_empty() { color("dim", "(not set)") } else { g_name.clone() }
    );
    println!(
        "    email: {}",
        if g_email.is_empty() { color("dim", "(not set)") } else { g_email.clone() }
    );

    if in_git_repo() {
        let l_name = get_git_config("user.name", "local");
        let l_email = get_git_config("user.email", "local");
        let remote = get_remote_url("origin");
        println!("\n  {}  ({})", color("bold", "Repo identity"), color("dim", &repo_name()));
        println!(
            "    name  : {}",
            if l_name.is_empty() { color("dim", "(inherits global)") } else { l_name }
        );
        println!(
            "    email : {}",
            if l_email.is_empty() { color("dim", "(inherits global)") } else { l_email }
        );
        println!(
            "    origin: {}",
            if remote.is_empty() { color("dim", "(no remote)") } else { remote }
        );
    } else {
        println!("\n  {}", color("dim", "(not in a git repository)"));
    }

    let result = Command::new("ssh-add")
        .arg("-l")
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output();
    println!("\n  {}", color("bold", "ssh-agent keys"));
    match result {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let lines: Vec<&str> = stdout.trim().lines().collect();
            if lines.is_empty() {
                println!("    {}", color("dim", "(no keys loaded, or agent not running)"));
            } else {
                for line in lines {
                    println!("    {} {}", color("green", "OK"), line);
                }
            }
        }
        _ => println!("    {}", color("dim", "(no keys loaded, or agent not running)")),
    }

    let active_email = if in_git_repo() {
        let local = get_git_config("user.email", "local");
        if local.is_empty() { g_email.clone() } else { local }
    } else {
        g_email.clone()
    };

    if !active_email.is_empty() {
        let accounts = load_accounts();
        let matched: Vec<&Account> = accounts.iter().filter(|a| a.email == active_email).collect();
        if let Some(m) = matched.first() {
            let host = if m.host.is_empty() { "github.com" } else { &m.host };
            println!(
                "\n  {}: {}  {}",
                color("bold", "Matched account"),
                color("green", &m.username),
                color("dim", host)
            );
        } else {
            println!("\n  {}", color("dim", "Active email does not match any configured account"));
        }
    }
    println!();
}

// Command: completions

fn cmd_completions(shell: Shell) {
    let mut cmd = Cli::command();
    generate(shell, &mut cmd, "git-id", &mut io::stdout());
}

// Main

fn main() {
    let cli = Cli::parse();
    let dry_run = cli.dry_run;

    match cli.command {
        Commands::Add => cmd_add(dry_run),
        Commands::List => cmd_list(),
        Commands::Use { username, global, force_ssh, force_https } => {
            cmd_use(&username, global, force_ssh, force_https, dry_run);
        }
        Commands::Remove { username, yes, delete_keys } => {
            cmd_remove(&username, yes, delete_keys, dry_run);
        }
        Commands::Ssh { subcommand } => match subcommand {
            SshCommands::Gen { username } => cmd_ssh_gen(&username, dry_run),
            SshCommands::Pick { username } => cmd_ssh_pick(&username, dry_run),
            SshCommands::Config => cmd_ssh_config(dry_run),
        },
        Commands::Status => cmd_status(),
        Commands::Completions { shell } => cmd_completions(shell),
    }
}
