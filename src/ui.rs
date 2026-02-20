use std::path::{Path, PathBuf};

pub fn is_tty() -> bool {
    use std::io::IsTerminal;
    std::io::stdout().is_terminal()
}

pub fn color(code: &str, text: &str) -> String {
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

pub fn print_ok(msg: &str) {
    println!("{} {}", color("green", "OK"), msg);
}

pub fn print_warn(msg: &str) {
    eprintln!("{} {}", color("yellow", "!"), msg);
}

pub fn print_err(msg: &str) {
    eprintln!("{} {}", color("red", "ERR"), msg);
}

pub fn print_info(msg: &str) {
    println!("{} {}", color("cyan", "->"), msg);
}

pub fn print_hdr(msg: &str) {
    println!("\n{}", color("bold", msg));
}

pub fn die(msg: &str, code: i32) -> ! {
    print_err(msg);
    std::process::exit(code);
}

pub fn backup(path: &Path) -> Option<PathBuf> {
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
