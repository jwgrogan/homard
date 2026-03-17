use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::Result;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    Global,
    Project,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInfo {
    pub name: String,
    pub path: PathBuf,
    pub scope: Scope,
    pub description: Option<String>,
    pub model: Option<String>,
    pub tools: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandInfo {
    pub name: String,
    pub path: PathBuf,
    pub scope: Scope,
    pub description: Option<String>,
}

// ---------------------------------------------------------------------------
// Frontmatter parsing (no YAML library — simple string splitting)
// ---------------------------------------------------------------------------

/// Extracts the YAML frontmatter block from Markdown content (between `---` markers).
/// Returns `None` if no frontmatter is present.
fn extract_frontmatter(content: &str) -> Option<&str> {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return None;
    }
    // Skip the opening `---` line
    let rest = trimmed.strip_prefix("---")?;
    // Allow optional whitespace/newline after `---`
    let rest = rest.trim_start_matches('\r').trim_start_matches('\n');
    // Find the closing `---`
    let end = rest.find("\n---")?;
    Some(&rest[..end])
}

/// Parse a single scalar string value for `key: value` in frontmatter text.
fn parse_scalar<'a>(frontmatter: &'a str, key: &str) -> Option<&'a str> {
    for line in frontmatter.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix(key) {
            if let Some(rest) = rest.strip_prefix(':') {
                let value = rest.trim().trim_matches('"').trim_matches('\'');
                if !value.is_empty() {
                    return Some(value);
                }
            }
        }
    }
    None
}

/// Parse a YAML sequence for `key` in frontmatter.
/// Handles both inline (`key: [a, b]`) and block list (`key:\n  - a\n  - b`) forms.
fn parse_sequence(frontmatter: &str, key: &str) -> Vec<String> {
    let lines: Vec<&str> = frontmatter.lines().collect();

    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix(key) {
            if let Some(rest) = rest.strip_prefix(':') {
                let value = rest.trim();
                // Inline list: key: [a, b, c]
                if value.starts_with('[') {
                    let inner = value
                        .trim_start_matches('[')
                        .trim_end_matches(']');
                    return inner
                        .split(',')
                        .map(|s| {
                            s.trim()
                                .trim_matches('"')
                                .trim_matches('\'')
                                .to_string()
                        })
                        .filter(|s| !s.is_empty())
                        .collect();
                }
                // Block list: following lines starting with `  - `
                let mut items = Vec::new();
                for subsequent in lines.iter().skip(i + 1) {
                    let sub = subsequent.trim();
                    if let Some(item) = sub.strip_prefix("- ") {
                        items.push(
                            item.trim()
                                .trim_matches('"')
                                .trim_matches('\'')
                                .to_string(),
                        );
                    } else if !sub.is_empty() {
                        // Non-list, non-empty line — end of sequence
                        break;
                    }
                }
                return items;
            }
        }
    }

    Vec::new()
}

// ---------------------------------------------------------------------------
// Agent discovery
// ---------------------------------------------------------------------------

/// Scan `dir` for `*.md` files (non-recursive) and parse each as an `AgentInfo`.
fn scan_agents(dir: &Path, scope: Scope) -> Result<Vec<AgentInfo>> {
    let mut agents = Vec::new();

    let read_dir = match std::fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(agents),
        Err(e) => return Err(crate::error::ArcctlError::Io(e)),
    };

    for entry in read_dir {
        let entry = entry.map_err(crate::error::ArcctlError::Io)?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }

        let content = std::fs::read_to_string(&path)
            .map_err(crate::error::ArcctlError::Io)?;

        let (description, model, tools, name_from_fm) =
            if let Some(fm) = extract_frontmatter(&content) {
                let desc = parse_scalar(fm, "description").map(String::from);
                let model = parse_scalar(fm, "model").map(String::from);
                // Claude uses both `tools` and `allowedTools`
                let tools = {
                    let t = parse_sequence(fm, "tools");
                    if t.is_empty() {
                        parse_sequence(fm, "allowedTools")
                    } else {
                        t
                    }
                };
                let name = parse_scalar(fm, "name").map(String::from);
                (desc, model, tools, name)
            } else {
                (None, None, Vec::new(), None)
            };

        // Prefer frontmatter name; fall back to file stem
        let name = name_from_fm.unwrap_or_else(|| {
            path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string()
        });

        agents.push(AgentInfo {
            name,
            path,
            scope: scope.clone(),
            description,
            model,
            tools,
        });
    }

    Ok(agents)
}

/// Discover agents from `global_dir` and optionally `project_dir`.
pub fn discover_agents(
    global_dir: &Path,
    project_dir: Option<&Path>,
) -> Result<Vec<AgentInfo>> {
    let mut agents = scan_agents(global_dir, Scope::Global)?;
    if let Some(pd) = project_dir {
        agents.extend(scan_agents(pd, Scope::Project)?);
    }
    Ok(agents)
}

// ---------------------------------------------------------------------------
// Command discovery
// ---------------------------------------------------------------------------

/// Recursively scan `dir` for `*.md` files. `name` is the relative path
/// from `base_dir` without the `.md` extension (e.g., `git/status`).
fn scan_commands_recursive(
    dir: &Path,
    base_dir: &Path,
    scope: Scope,
    out: &mut Vec<CommandInfo>,
) -> Result<()> {
    let read_dir = match std::fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(crate::error::ArcctlError::Io(e)),
    };

    for entry in read_dir {
        let entry = entry.map_err(crate::error::ArcctlError::Io)?;
        let path = entry.path();

        if path.is_dir() {
            scan_commands_recursive(&path, base_dir, scope.clone(), out)?;
            continue;
        }

        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }

        let content = std::fs::read_to_string(&path)
            .map_err(crate::error::ArcctlError::Io)?;

        let description = extract_frontmatter(&content)
            .and_then(|fm| parse_scalar(fm, "description"))
            .map(String::from);

        // Build namespaced name from relative path without .md
        let rel = path
            .strip_prefix(base_dir)
            .unwrap_or(&path);
        let name = rel
            .with_extension("")
            .to_string_lossy()
            .replace('\\', "/"); // normalise on Windows (noop on Unix)

        out.push(CommandInfo {
            name,
            path,
            scope: scope.clone(),
            description,
        });
    }

    Ok(())
}

/// Discover commands from `global_dir` and optionally `project_dir`.
pub fn discover_commands(
    global_dir: &Path,
    project_dir: Option<&Path>,
) -> Result<Vec<CommandInfo>> {
    let mut commands = Vec::new();
    scan_commands_recursive(global_dir, global_dir, Scope::Global, &mut commands)?;
    if let Some(pd) = project_dir {
        scan_commands_recursive(pd, pd, Scope::Project, &mut commands)?;
    }
    Ok(commands)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    // Helper: write a file and create parent dirs
    fn write_file(path: &Path, content: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, content).unwrap();
    }

    // ---------------------------------------------------------------------------
    // Agent tests
    // ---------------------------------------------------------------------------

    #[test]
    fn test_discover_agents_empty_dir() {
        let dir = TempDir::new().unwrap();
        let agents = discover_agents(dir.path(), None).unwrap();
        assert!(agents.is_empty());
    }

    #[test]
    fn test_discover_agents_missing_dir() {
        let dir = TempDir::new().unwrap();
        let missing = dir.path().join("nonexistent");
        let agents = discover_agents(&missing, None).unwrap();
        assert!(agents.is_empty());
    }

    #[test]
    fn test_discover_agents_basic_name() {
        let dir = TempDir::new().unwrap();
        write_file(&dir.path().join("my-agent.md"), "# My agent\n\nHello");

        let agents = discover_agents(dir.path(), None).unwrap();
        assert_eq!(agents.len(), 1);
        assert_eq!(agents[0].name, "my-agent");
        assert_eq!(agents[0].scope, Scope::Global);
    }

    #[test]
    fn test_discover_agents_parses_frontmatter() {
        let dir = TempDir::new().unwrap();
        let content = "---\nname: my-agent\ndescription: Does things\nmodel: claude-opus-4\ntools: [Bash, Read]\n---\n\nBody text.";
        write_file(&dir.path().join("agent.md"), content);

        let agents = discover_agents(dir.path(), None).unwrap();
        assert_eq!(agents.len(), 1);
        let a = &agents[0];
        assert_eq!(a.name, "my-agent");
        assert_eq!(a.description.as_deref(), Some("Does things"));
        assert_eq!(a.model.as_deref(), Some("claude-opus-4"));
        assert_eq!(a.tools, vec!["Bash", "Read"]);
    }

    #[test]
    fn test_discover_agents_allowed_tools_alias() {
        let dir = TempDir::new().unwrap();
        let content = "---\nallowedTools:\n  - Write\n  - Edit\n---\n";
        write_file(&dir.path().join("agent.md"), content);

        let agents = discover_agents(dir.path(), None).unwrap();
        assert_eq!(agents[0].tools, vec!["Write", "Edit"]);
    }

    #[test]
    fn test_discover_agents_non_md_ignored() {
        let dir = TempDir::new().unwrap();
        write_file(&dir.path().join("agent.md"), "# Agent");
        write_file(&dir.path().join("readme.txt"), "not an agent");
        write_file(&dir.path().join("config.json"), "{}");

        let agents = discover_agents(dir.path(), None).unwrap();
        assert_eq!(agents.len(), 1);
    }

    #[test]
    fn test_discover_agents_merges_global_and_project() {
        let global = TempDir::new().unwrap();
        let project = TempDir::new().unwrap();

        write_file(&global.path().join("global-agent.md"), "# Global");
        write_file(&project.path().join("project-agent.md"), "# Project");

        let agents = discover_agents(global.path(), Some(project.path())).unwrap();
        assert_eq!(agents.len(), 2);

        let global_agent = agents.iter().find(|a| a.name == "global-agent").unwrap();
        let project_agent = agents.iter().find(|a| a.name == "project-agent").unwrap();
        assert_eq!(global_agent.scope, Scope::Global);
        assert_eq!(project_agent.scope, Scope::Project);
    }

    // ---------------------------------------------------------------------------
    // Command tests
    // ---------------------------------------------------------------------------

    #[test]
    fn test_discover_commands_empty_dir() {
        let dir = TempDir::new().unwrap();
        let commands = discover_commands(dir.path(), None).unwrap();
        assert!(commands.is_empty());
    }

    #[test]
    fn test_discover_commands_missing_dir() {
        let dir = TempDir::new().unwrap();
        let missing = dir.path().join("nonexistent");
        let commands = discover_commands(&missing, None).unwrap();
        assert!(commands.is_empty());
    }

    #[test]
    fn test_discover_commands_basic_name() {
        let dir = TempDir::new().unwrap();
        write_file(&dir.path().join("deploy.md"), "# Deploy");

        let commands = discover_commands(dir.path(), None).unwrap();
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].name, "deploy");
        assert_eq!(commands[0].scope, Scope::Global);
    }

    #[test]
    fn test_discover_commands_nested_namespace() {
        let dir = TempDir::new().unwrap();
        write_file(&dir.path().join("git").join("status.md"), "# Git status");
        write_file(&dir.path().join("git").join("commit.md"), "# Git commit");
        write_file(&dir.path().join("top.md"), "# Top");

        let commands = discover_commands(dir.path(), None).unwrap();
        assert_eq!(commands.len(), 3);

        let names: Vec<&str> = commands.iter().map(|c| c.name.as_str()).collect();
        assert!(names.contains(&"git/status"));
        assert!(names.contains(&"git/commit"));
        assert!(names.contains(&"top"));
    }

    #[test]
    fn test_discover_commands_non_md_ignored() {
        let dir = TempDir::new().unwrap();
        write_file(&dir.path().join("cmd.md"), "# Cmd");
        write_file(&dir.path().join("notes.txt"), "ignore me");

        let commands = discover_commands(dir.path(), None).unwrap();
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].name, "cmd");
    }

    #[test]
    fn test_discover_commands_parses_description() {
        let dir = TempDir::new().unwrap();
        let content = "---\ndescription: Run tests\n---\n\nBody.";
        write_file(&dir.path().join("test.md"), content);

        let commands = discover_commands(dir.path(), None).unwrap();
        assert_eq!(commands[0].description.as_deref(), Some("Run tests"));
    }

    #[test]
    fn test_discover_commands_merges_global_and_project() {
        let global = TempDir::new().unwrap();
        let project = TempDir::new().unwrap();

        write_file(&global.path().join("global-cmd.md"), "# Global");
        write_file(&project.path().join("project-cmd.md"), "# Project");

        let commands = discover_commands(global.path(), Some(project.path())).unwrap();
        assert_eq!(commands.len(), 2);

        let global_cmd = commands.iter().find(|c| c.name == "global-cmd").unwrap();
        let project_cmd = commands.iter().find(|c| c.name == "project-cmd").unwrap();
        assert_eq!(global_cmd.scope, Scope::Global);
        assert_eq!(project_cmd.scope, Scope::Project);
    }

    // ---------------------------------------------------------------------------
    // Frontmatter parsing unit tests
    // ---------------------------------------------------------------------------

    #[test]
    fn test_parse_scalar_basic() {
        let fm = "name: hello\ndescription: A test\n";
        assert_eq!(parse_scalar(fm, "name"), Some("hello"));
        assert_eq!(parse_scalar(fm, "description"), Some("A test"));
        assert_eq!(parse_scalar(fm, "missing"), None);
    }

    #[test]
    fn test_parse_sequence_inline() {
        let fm = "tools: [Bash, Read, Write]\n";
        let tools = parse_sequence(fm, "tools");
        assert_eq!(tools, vec!["Bash", "Read", "Write"]);
    }

    #[test]
    fn test_parse_sequence_block() {
        let fm = "tools:\n  - Bash\n  - Read\n";
        let tools = parse_sequence(fm, "tools");
        assert_eq!(tools, vec!["Bash", "Read"]);
    }

    #[test]
    fn test_extract_frontmatter_none_when_absent() {
        assert!(extract_frontmatter("# Just markdown\n\nNo frontmatter.").is_none());
        assert!(extract_frontmatter("").is_none());
    }

    #[test]
    fn test_extract_frontmatter_present() {
        let content = "---\nname: test\n---\n\nBody.";
        let fm = extract_frontmatter(content);
        assert!(fm.is_some());
        assert!(fm.unwrap().contains("name: test"));
    }
}
