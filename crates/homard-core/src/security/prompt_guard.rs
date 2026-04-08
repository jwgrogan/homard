use regex_lite::Regex;

const INJECTION_PATTERNS: &[&str] = &[
    r"(?i)ignore\s+previous\s+instructions",
    r"(?i)you\s+are\s+now",
    r"(?i)system\s*:\s*",
    r"(?i)forget\s+everything",
];

pub fn check_output(output: &str) -> Option<String> {
    for pattern in INJECTION_PATTERNS {
        if let Ok(re) = Regex::new(pattern) {
            if re.is_match(output) {
                return Some(format!(
                    "<tool_result trust=\"untrusted\">\nPotential prompt injection detected.\n{}\n</tool_result>",
                    output
                ));
            }
        }
    }
    None
}
