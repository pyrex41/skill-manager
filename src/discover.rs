use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::Result;
use walkdir::WalkDir;

/// Represents an installed skill discovered in the current directory
#[derive(Debug, Clone)]
pub struct InstalledSkill {
    /// The name of the skill (derived from filename)
    pub name: String,
    /// The type of skill (skill, agent, command)
    pub skill_type: SkillType,
    /// The tool this is installed for
    pub tool: InstalledTool,
    /// Full path to the skill file
    pub path: PathBuf,
    /// Optional bundle name (if detectable from path structure)
    pub bundle: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InstalledTool {
    Claude,
    OpenCode,
    Cursor,
}

impl InstalledTool {
    pub fn as_str(&self) -> &'static str {
        match self {
            InstalledTool::Claude => "claude",
            InstalledTool::OpenCode => "opencode",
            InstalledTool::Cursor => "cursor",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            InstalledTool::Claude => "Claude",
            InstalledTool::OpenCode => "OpenCode",
            InstalledTool::Cursor => "Cursor",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SkillType {
    Skill,
    Agent,
    Command,
    Rule,
}

impl SkillType {
    pub fn plural(&self) -> &'static str {
        match self {
            SkillType::Skill => "skills",
            SkillType::Agent => "agents",
            SkillType::Command => "commands",
            SkillType::Rule => "rules",
        }
    }
}

/// Discover all installed skills in a directory
pub fn discover_installed(base: &Path) -> Result<Vec<InstalledSkill>> {
    let mut skills = Vec::new();

    // Discover Claude skills
    skills.extend(discover_claude(base)?);

    // Discover OpenCode skills
    skills.extend(discover_opencode(base)?);

    // Discover Cursor skills
    skills.extend(discover_cursor(base)?);

    Ok(skills)
}

/// Discover Claude installed skills
fn discover_claude(base: &Path) -> Result<Vec<InstalledSkill>> {
    let mut skills = Vec::new();
    let claude_dir = base.join(".claude");

    if !claude_dir.exists() {
        return Ok(skills);
    }

    // .claude/commands/**/*.md -> commands
    let commands_dir = claude_dir.join("commands");
    if commands_dir.exists() {
        for entry in WalkDir::new(&commands_dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| e.path().extension().map(|ext| ext == "md").unwrap_or(false))
        {
            let path = entry.path().to_path_buf();
            let name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();

            // Try to detect bundle from path: .claude/commands/bundle/skill.md
            let bundle = path.parent().and_then(|p| {
                if p != commands_dir {
                    p.file_name().and_then(|n| n.to_str()).map(String::from)
                } else {
                    None
                }
            });

            if !name.is_empty() {
                skills.push(InstalledSkill {
                    name,
                    skill_type: SkillType::Command,
                    tool: InstalledTool::Claude,
                    path,
                    bundle,
                });
            }
        }
    }

    // .claude/agents/**/*.md -> agents
    let agents_dir = claude_dir.join("agents");
    if agents_dir.exists() {
        for entry in WalkDir::new(&agents_dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| e.path().extension().map(|ext| ext == "md").unwrap_or(false))
        {
            let path = entry.path().to_path_buf();
            let name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();

            let bundle = path.parent().and_then(|p| {
                if p != agents_dir {
                    p.file_name().and_then(|n| n.to_str()).map(String::from)
                } else {
                    None
                }
            });

            if !name.is_empty() {
                skills.push(InstalledSkill {
                    name,
                    skill_type: SkillType::Agent,
                    tool: InstalledTool::Claude,
                    path,
                    bundle,
                });
            }
        }
    }

    Ok(skills)
}

/// Discover OpenCode installed skills
fn discover_opencode(base: &Path) -> Result<Vec<InstalledSkill>> {
    let mut skills = Vec::new();
    let opencode_dir = base.join(".opencode");

    if !opencode_dir.exists() {
        return Ok(skills);
    }

    // .opencode/skill/*/SKILL.md -> skills
    let skill_dir = opencode_dir.join("skill");
    if skill_dir.exists() {
        for entry in std::fs::read_dir(&skill_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                let skill_file = path.join("SKILL.md");
                if skill_file.exists() {
                    let name = path
                        .file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or("")
                        .to_string();

                    if !name.is_empty() {
                        skills.push(InstalledSkill {
                            name: name.clone(),
                            skill_type: SkillType::Skill,
                            tool: InstalledTool::OpenCode,
                            path: skill_file,
                            bundle: Some(name),
                        });
                    }
                }
            }
        }
    }

    // .opencode/agent/*.md -> agents
    let agent_dir = opencode_dir.join("agent");
    if agent_dir.exists() {
        for entry in std::fs::read_dir(&agent_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() && path.extension().map(|e| e == "md").unwrap_or(false) {
                let name = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_string();

                if !name.is_empty() {
                    skills.push(InstalledSkill {
                        name,
                        skill_type: SkillType::Agent,
                        tool: InstalledTool::OpenCode,
                        path,
                        bundle: None,
                    });
                }
            }
        }
    }

    // .opencode/command/*.md -> commands
    let command_dir = opencode_dir.join("command");
    if command_dir.exists() {
        for entry in std::fs::read_dir(&command_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() && path.extension().map(|e| e == "md").unwrap_or(false) {
                let name = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_string();

                if !name.is_empty() {
                    skills.push(InstalledSkill {
                        name,
                        skill_type: SkillType::Command,
                        tool: InstalledTool::OpenCode,
                        path,
                        bundle: None,
                    });
                }
            }
        }
    }

    Ok(skills)
}

/// Discover Cursor installed skills
fn discover_cursor(base: &Path) -> Result<Vec<InstalledSkill>> {
    let mut skills = Vec::new();
    let cursor_dir = base.join(".cursor");

    if !cursor_dir.exists() {
        return Ok(skills);
    }

    // .cursor/skills/*/SKILL.md -> skills
    let skills_dir = cursor_dir.join("skills");
    if skills_dir.exists() {
        for entry in std::fs::read_dir(&skills_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                let skill_file = path.join("SKILL.md");
                if skill_file.exists() {
                    let name = path
                        .file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or("")
                        .to_string();

                    if !name.is_empty() {
                        skills.push(InstalledSkill {
                            name: name.clone(),
                            skill_type: SkillType::Skill,
                            tool: InstalledTool::Cursor,
                            path: skill_file,
                            bundle: Some(name),
                        });
                    }
                }
            }
        }
    }

    // .cursor/rules/*/RULE.md -> rules (folder-based)
    let rules_dir = cursor_dir.join("rules");
    if rules_dir.exists() {
        for entry in std::fs::read_dir(&rules_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                let rule_file = path.join("RULE.md");
                if rule_file.exists() {
                    let name = path
                        .file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or("")
                        .to_string();

                    if !name.is_empty() {
                        skills.push(InstalledSkill {
                            name: name.clone(),
                            skill_type: SkillType::Rule,
                            tool: InstalledTool::Cursor,
                            path: rule_file,
                            bundle: Some(name),
                        });
                    }
                }
            }
        }
    }

    Ok(skills)
}

/// Group skills by tool, then by type
pub fn group_by_tool(
    skills: &[InstalledSkill],
) -> HashMap<InstalledTool, HashMap<SkillType, Vec<&InstalledSkill>>> {
    let mut result: HashMap<InstalledTool, HashMap<SkillType, Vec<&InstalledSkill>>> =
        HashMap::new();

    for skill in skills {
        result
            .entry(skill.tool)
            .or_default()
            .entry(skill.skill_type)
            .or_default()
            .push(skill);
    }

    result
}

/// Filter skills to a specific tool
pub fn filter_by_tool(skills: Vec<InstalledSkill>, tool: &str) -> Vec<InstalledSkill> {
    let tool_lower = tool.to_lowercase();
    skills
        .into_iter()
        .filter(|s| s.tool.as_str() == tool_lower)
        .collect()
}

/// Get a unique identifier for a skill (for grouping across tools)
impl InstalledSkill {
    pub fn unique_id(&self) -> String {
        if let Some(ref bundle) = self.bundle {
            format!("{}/{}", bundle, self.name)
        } else {
            self.name.clone()
        }
    }
}

/// Group skills that have the same name/bundle across different tools
pub fn group_same_skills(skills: &[InstalledSkill]) -> HashMap<String, Vec<&InstalledSkill>> {
    let mut result: HashMap<String, Vec<&InstalledSkill>> = HashMap::new();

    for skill in skills {
        result.entry(skill.unique_id()).or_default().push(skill);
    }

    result
}

/// Remove a skill file and clean up empty parent directories
pub fn remove_skill(skill: &InstalledSkill) -> Result<()> {
    // For skills/rules that are directories (OpenCode/Cursor skills/rules), remove the whole directory
    if skill.skill_type == SkillType::Skill || skill.skill_type == SkillType::Rule {
        if let Some(parent) = skill.path.parent() {
            if parent.is_dir() {
                std::fs::remove_dir_all(parent)?;
                return Ok(());
            }
        }
    }

    // Remove the file
    std::fs::remove_file(&skill.path)?;

    // Clean up empty parent directories
    let mut current = skill.path.parent();
    while let Some(parent) = current {
        // Stop at the tool directory (.claude, .opencode, .cursor)
        if let Some(name) = parent.file_name().and_then(|n| n.to_str()) {
            if name.starts_with('.') {
                break;
            }
        }

        // Try to remove if empty
        if std::fs::read_dir(parent)?.next().is_none() {
            std::fs::remove_dir(parent)?;
            current = parent.parent();
        } else {
            break;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_discover_empty_dir() {
        let dir = tempdir().unwrap();
        let skills = discover_installed(dir.path()).unwrap();
        assert!(skills.is_empty());
    }

    #[test]
    fn test_discover_claude_commands() {
        let dir = tempdir().unwrap();

        // Create .claude/commands/test.md
        let commands_dir = dir.path().join(".claude/commands");
        fs::create_dir_all(&commands_dir).unwrap();
        fs::write(commands_dir.join("test.md"), "# Test command").unwrap();

        let skills = discover_installed(dir.path()).unwrap();
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].name, "test");
        assert_eq!(skills[0].skill_type, SkillType::Command);
        assert_eq!(skills[0].tool, InstalledTool::Claude);
    }

    #[test]
    fn test_discover_claude_commands_with_bundle() {
        let dir = tempdir().unwrap();

        // Create .claude/commands/mybundle/test.md
        let bundle_dir = dir.path().join(".claude/commands/mybundle");
        fs::create_dir_all(&bundle_dir).unwrap();
        fs::write(bundle_dir.join("test.md"), "# Test command").unwrap();

        let skills = discover_installed(dir.path()).unwrap();
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].name, "test");
        assert_eq!(skills[0].bundle, Some("mybundle".to_string()));
    }

    #[test]
    fn test_discover_opencode_skills() {
        let dir = tempdir().unwrap();

        // Create .opencode/skill/myskill/SKILL.md
        let skill_dir = dir.path().join(".opencode/skill/myskill");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(skill_dir.join("SKILL.md"), "# My skill").unwrap();

        let skills = discover_installed(dir.path()).unwrap();
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].name, "myskill");
        assert_eq!(skills[0].skill_type, SkillType::Skill);
        assert_eq!(skills[0].tool, InstalledTool::OpenCode);
    }

    #[test]
    fn test_discover_cursor_rules() {
        let dir = tempdir().unwrap();

        // Create .cursor/rules/test/RULE.md (folder-based)
        let rule_dir = dir.path().join(".cursor/rules/test");
        fs::create_dir_all(&rule_dir).unwrap();
        fs::write(rule_dir.join("RULE.md"), "# Test rule").unwrap();

        let skills = discover_installed(dir.path()).unwrap();
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].name, "test");
        assert_eq!(skills[0].skill_type, SkillType::Rule);
        assert_eq!(skills[0].tool, InstalledTool::Cursor);
    }

    #[test]
    fn test_filter_by_tool() {
        let skills = vec![
            InstalledSkill {
                name: "test1".to_string(),
                skill_type: SkillType::Command,
                tool: InstalledTool::Claude,
                path: PathBuf::from("/test1"),
                bundle: None,
            },
            InstalledSkill {
                name: "test2".to_string(),
                skill_type: SkillType::Command,
                tool: InstalledTool::OpenCode,
                path: PathBuf::from("/test2"),
                bundle: None,
            },
        ];

        let filtered = filter_by_tool(skills, "claude");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "test1");
    }
}
