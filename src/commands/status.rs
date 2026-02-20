use crate::config::load_accounts;
use crate::git::{get_git_config, get_remote_url, in_git_repo, repo_name};
use crate::ui::{color, print_hdr};
use std::process::{Command, Stdio};

pub fn cmd_status() {
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

    print_ssh_agent_keys();

    let active_email = if in_git_repo() {
        let local = get_git_config("user.email", "local");
        if local.is_empty() { g_email.clone() } else { local }
    } else {
        g_email.clone()
    };

    if !active_email.is_empty() {
        let accounts = load_accounts();
        let matched: Vec<_> = accounts.iter().filter(|a| a.email == active_email).collect();
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

fn print_ssh_agent_keys() {
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
}
