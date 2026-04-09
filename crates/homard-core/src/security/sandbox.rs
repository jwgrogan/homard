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

    // Normalize: collapse whitespace, trim
    let normalized: String = command.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase();

    // Block shell metacharacters that enable evasion
    if normalized.contains("$(") || normalized.contains("` ") || normalized.contains("`\n") {
        return true;
    }
    // Block backtick command substitution
    if command.contains('`') {
        return true;
    }

    // Check pipe-to-shell patterns
    if (normalized.contains("curl") || normalized.contains("wget"))
        && (normalized.contains("| sh") || normalized.contains("| bash") || normalized.contains("| zsh")
            || normalized.contains("|sh") || normalized.contains("|bash") || normalized.contains("|zsh")) {
        return true;
    }

    BLOCKED_PATTERNS.iter().any(|p| normalized.contains(&p.to_lowercase()))
}

pub fn needs_confirmation(arguments: &serde_json::Value) -> bool {
    let command = arguments.get("command")
        .and_then(|c| c.as_str())
        .unwrap_or("");

    let lower = command.to_lowercase();
    CONFIRM_PATTERNS.iter().any(|p| lower.contains(&p.to_lowercase()))
}
