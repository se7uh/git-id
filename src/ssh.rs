use crate::config::{account_id, ssh_host_alias};
use crate::models::Account;
use crate::ui::{backup, die, print_info, print_ok, print_warn};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub fn ssh_dir() -> PathBuf {
    crate::config::dirs_home().join(".ssh")
}

pub fn ssh_config_path() -> PathBuf {
    crate::config::dirs_home().join(".ssh").join("config")
}

fn default_key_path(username: &str) -> PathBuf {
    ssh_dir().join(format!("id_ed25519_{username}"))
}

pub const MARKER_S: &str = "# >>> git-id: {id} >>>";
pub const MARKER_E: &str = "# <<< git-id: {id} <<<";

pub fn make_stanza(acc: &Account) -> String {
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

pub fn update_ssh_config(accounts: &[Account], dry_run: bool) {
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

pub fn replace_stanza(content: &str, start: &str, end: &str, replacement: &str) -> String {
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

pub fn remove_stanza(content: &str, start: &str, end: &str) -> String {
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

pub fn gen_ssh_key(username: &str, email: &str, dry_run: bool) -> PathBuf {
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

pub fn add_key_to_agent(key: &Path, dry_run: bool) {
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

pub fn fix_key_permissions(key: &Path) {
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
