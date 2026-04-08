const BLOCKED_PATTERNS: &[&str] = &[
    "rm -rf /",
    "rm -rf ~",
    "rm -rf $HOME",
    "mkfs",
    "dd if=",
    "> /dev/sd",
    "> /dev/disk",
    ":(){ :|:&",
    "chmod 777",
    "chmod -R 777",
    "curl|sh",
    "curl|bash",
    "wget|sh",
    "wget|bash",
    "python -c",
    "python3 -c",
    "base64 -d|sh",
    "base64 -d|bash",
    "eval ",
    "/etc/passwd",
    "/etc/shadow",
    "launchctl",
    "diskutil",
    "csrutil",
    "nvram",
    "bless ",
    "shutdown",
    "reboot",
    "halt ",
];

/// Patterns that require user approval in supervised mode
const CONFIRM_PATTERNS: &[&str] = &[
    "rm -rf",
    "rm -r",
    "sudo ",
    "pip install",
    "npm install -g",
    "brew install",
    "brew uninstall",
    "git push",
    "git reset",
    "git checkout .",
    "docker rm",
    "docker rmi",
    "kill ",
    "killall ",
    "pkill ",
    "chmod ",
    "chown ",
    "mv /",
    "cp /",
];

pub fn is_blocked(arguments: &serde_json::Value) -> bool {
    let command = arguments.get("command")
        .and_then(|c| c.as_str())
        .unwrap_or("");

    let lower = command.to_lowercase();

    // Check for pipe-to-shell patterns (common injection)
    if lower.contains("|sh") || lower.contains("|bash") || lower.contains("|zsh") {
        if lower.contains("curl") || lower.contains("wget") {
            return true;
        }
    }

    BLOCKED_PATTERNS.iter().any(|p| lower.contains(&p.to_lowercase()))
}

pub fn needs_confirmation(arguments: &serde_json::Value) -> bool {
    let command = arguments.get("command")
        .and_then(|c| c.as_str())
        .unwrap_or("");

    let lower = command.to_lowercase();
    CONFIRM_PATTERNS.iter().any(|p| lower.contains(&p.to_lowercase()))
}
