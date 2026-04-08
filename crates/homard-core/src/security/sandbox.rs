const BLOCKED_PATTERNS: &[&str] = &[
    "rm -rf /",
    "rm -rf ~",
    "mkfs",
    "dd if=",
    "> /dev/sd",
    ":(){ :|:&",
];

pub fn is_blocked(arguments: &serde_json::Value) -> bool {
    let command = arguments.get("command")
        .and_then(|c| c.as_str())
        .unwrap_or("");

    let lower = command.to_lowercase();
    BLOCKED_PATTERNS.iter().any(|p| lower.contains(p))
}
