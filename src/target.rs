use anyhow::Result;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

use crate::bundle::{SkillFile, SkillType};

/// Target AI coding tool
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    Claude,
    OpenCode,
    Cursor,
}

impl Tool {
    /// Get the global install target for this tool
    pub fn global_target(&self) -> PathBuf {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));

        match self {
            Tool::Claude => home,
            Tool::OpenCode => home.join(".config/opencode"),
            Tool::Cursor => {
                eprintln!("Warning: Cursor doesn't support global config, using current directory");
                std::env::current_dir().unwrap_or(home)
            }
        }
    }

    /// Get the name of this tool for display
    pub fn name(&self) -> &'static str {
        match self {
            Tool::Claude => "Claude",
            Tool::OpenCode => "OpenCode",
            Tool::Cursor => "Cursor",
        }
    }

    /// Write a skill file to the appropriate location for this tool
    pub fn write_file(
        &self,
        target_dir: &PathBuf,
        bundle_name: &str,
        skill: &SkillFile,
    ) -> Result<PathBuf> {
        match self {
            Tool::Claude => self.write_claude(target_dir, bundle_name, skill),
            Tool::OpenCode => self.write_opencode(target_dir, bundle_name, skill),
            Tool::Cursor => self.write_cursor(target_dir, bundle_name, skill),
        }
    }

    /// Get the destination info string for display
    pub fn dest_info(&self, skill_type: SkillType, bundle_name: &str) -> String {
        match self {
            Tool::Claude => format!(".claude/{}/{}/", skill_type.dir_name(), bundle_name),
            Tool::OpenCode => match skill_type {
                SkillType::Skill => format!(".opencode/skill/{}-*/", bundle_name),
                SkillType::Agent => ".opencode/agent/".to_string(),
                SkillType::Command => ".opencode/command/".to_string(),
            },
            Tool::Cursor => match skill_type {
                SkillType::Skill => format!(".cursor/skills/{}-*/", bundle_name),
                _ => ".cursor/rules/".to_string(),
            },
        }
    }

    // Claude: .claude/{type}/{bundle}/{name}.md
    fn write_claude(
        &self,
        target_dir: &PathBuf,
        bundle_name: &str,
        skill: &SkillFile,
    ) -> Result<PathBuf> {
        let dest_dir = target_dir
            .join(".claude")
            .join(skill.skill_type.dir_name())
            .join(bundle_name);

        fs::create_dir_all(&dest_dir)?;

        let dest_file = dest_dir.join(format!("{}.md", skill.name));
        fs::copy(&skill.path, &dest_file)?;

        Ok(dest_file)
    }

    // OpenCode:
    //   skills -> .opencode/skill/{bundle}-{name}/SKILL.md (with frontmatter)
    //   agents -> .opencode/agent/{bundle}-{name}.md
    //   commands -> .opencode/command/{bundle}-{name}.md
    fn write_opencode(
        &self,
        target_dir: &PathBuf,
        bundle_name: &str,
        skill: &SkillFile,
    ) -> Result<PathBuf> {
        let combined_name = format!("{}-{}", bundle_name, skill.name);

        match skill.skill_type {
            SkillType::Skill => {
                let dest_dir = target_dir.join(".opencode/skill").join(&combined_name);
                fs::create_dir_all(&dest_dir)?;

                let dest_file = dest_dir.join("SKILL.md");
                transform_skill_file(&skill.path, &dest_file, &combined_name)?;

                Ok(dest_file)
            }
            SkillType::Agent => {
                let dest_dir = target_dir.join(".opencode/agent");
                fs::create_dir_all(&dest_dir)?;

                let dest_file = dest_dir.join(format!("{}.md", combined_name));
                fs::copy(&skill.path, &dest_file)?;

                Ok(dest_file)
            }
            SkillType::Command => {
                let dest_dir = target_dir.join(".opencode/command");
                fs::create_dir_all(&dest_dir)?;

                let dest_file = dest_dir.join(format!("{}.md", combined_name));
                fs::copy(&skill.path, &dest_file)?;

                Ok(dest_file)
            }
        }
    }

    // Cursor:
    //   skills -> .cursor/skills/{bundle}-{name}/SKILL.md (with frontmatter)
    //   agents/commands -> .cursor/rules/{bundle}-{name}.mdc
    fn write_cursor(
        &self,
        target_dir: &PathBuf,
        bundle_name: &str,
        skill: &SkillFile,
    ) -> Result<PathBuf> {
        let combined_name = format!("{}-{}", bundle_name, skill.name);

        match skill.skill_type {
            SkillType::Skill => {
                let dest_dir = target_dir.join(".cursor/skills").join(&combined_name);
                fs::create_dir_all(&dest_dir)?;

                let dest_file = dest_dir.join("SKILL.md");
                transform_skill_file(&skill.path, &dest_file, &combined_name)?;

                Ok(dest_file)
            }
            _ => {
                let dest_dir = target_dir.join(".cursor/rules");
                fs::create_dir_all(&dest_dir)?;

                // Note: .mdc extension for Cursor rules
                let dest_file = dest_dir.join(format!("{}.mdc", combined_name));
                fs::copy(&skill.path, &dest_file)?;

                Ok(dest_file)
            }
        }
    }
}

/// Transform a skill file to ensure it has proper frontmatter with name field
fn transform_skill_file(src: &PathBuf, dest: &PathBuf, skill_name: &str) -> Result<()> {
    let content = fs::read_to_string(src)?;
    let lines: Vec<&str> = content.lines().collect();

    let output = if lines.first() == Some(&"---") {
        // Has frontmatter - check if name exists in the frontmatter section
        let mut in_frontmatter = false;
        let mut has_name = false;

        for line in &lines {
            if *line == "---" {
                if in_frontmatter {
                    break; // End of frontmatter
                }
                in_frontmatter = true;
                continue;
            }
            if in_frontmatter && line.starts_with("name:") {
                has_name = true;
                break;
            }
        }

        if has_name {
            // Already has name, use as-is
            content
        } else {
            // Add name after first ---
            let mut result = String::new();
            result.push_str("---\n");
            result.push_str(&format!("name: {}\n", skill_name));

            // Skip the first "---" and add the rest
            for line in lines.iter().skip(1) {
                result.push_str(line);
                result.push('\n');
            }
            result
        }
    } else {
        // No frontmatter - add it
        let mut result = String::new();
        result.push_str("---\n");
        result.push_str(&format!("name: {}\n", skill_name));
        result.push_str("---\n");
        result.push_str(&content);
        result
    };

    let mut file = fs::File::create(dest)?;
    file.write_all(output.as_bytes())?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_tool_names() {
        assert_eq!(Tool::Claude.name(), "Claude");
        assert_eq!(Tool::OpenCode.name(), "OpenCode");
        assert_eq!(Tool::Cursor.name(), "Cursor");
    }

    #[test]
    fn test_transform_skill_no_frontmatter() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("src.md");
        let dest = dir.path().join("dest.md");

        fs::write(&src, "# My Skill\n\nContent here").unwrap();
        transform_skill_file(&src, &dest, "test-skill").unwrap();

        let result = fs::read_to_string(&dest).unwrap();
        assert!(result.starts_with("---\nname: test-skill\n---\n"));
        assert!(result.contains("# My Skill"));
    }

    #[test]
    fn test_transform_skill_with_frontmatter_no_name() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("src.md");
        let dest = dir.path().join("dest.md");

        fs::write(&src, "---\ndescription: test\n---\n# My Skill").unwrap();
        transform_skill_file(&src, &dest, "test-skill").unwrap();

        let result = fs::read_to_string(&dest).unwrap();
        assert!(result.contains("name: test-skill"));
        assert!(result.contains("description: test"));
    }

    #[test]
    fn test_transform_skill_with_name() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("src.md");
        let dest = dir.path().join("dest.md");

        fs::write(&src, "---\nname: existing-name\n---\n# My Skill").unwrap();
        transform_skill_file(&src, &dest, "test-skill").unwrap();

        let result = fs::read_to_string(&dest).unwrap();
        assert!(result.contains("name: existing-name"));
        assert!(!result.contains("name: test-skill"));
    }
}
