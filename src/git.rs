use crate::config::ssh_host_alias;
use crate::models::Account;
use crate::ui::{print_info, print_ok, print_warn};
use std::path::PathBuf;
use std::process::{Command, Stdio};

pub fn run_git(args: &[&str]) -> (i32, String, String) {
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

pub fn in_git_repo() -> bool {
    run_git(&["rev-parse", "--git-dir"]).0 == 0
}

pub fn get_git_config(key: &str, scope: &str) -> String {
    let flag = format!("--{scope}");
    let (code, out, _) = run_git(&["config", &flag, key]);
    if code == 0 { out } else { String::new() }
}

pub fn set_git_config(key: &str, value: &str, scope: &str, dry_run: bool) {
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

pub fn get_remote_url(remote: &str) -> String {
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

pub fn parse_remote_url(url: &str) -> Option<(String, String, String, String)> {
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

pub fn build_ssh_url(acc: &Account, owner: &str, repo: &str) -> String {
    let alias = ssh_host_alias(acc);
    format!("git@{alias}:{owner}/{repo}.git")
}

pub fn build_https_url(token: &str, host: &str, owner: &str, repo: &str) -> String {
    if !token.is_empty() {
        format!("https://{token}@{host}/{owner}/{repo}.git")
    } else {
        format!("https://{host}/{owner}/{repo}.git")
    }
}

pub fn set_remote_url(remote: &str, url: &str, dry_run: bool) {
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

pub fn repo_name() -> String {
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
