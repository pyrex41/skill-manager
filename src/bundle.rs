use serde::Deserialize;
use std::path::PathBuf;

/// Type of skill item
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillType {
    Skill,
    Agent,
    Command,
    Rule,
}

impl SkillType {
    pub fn dir_name(&self) -> &'static str {
        match self {
            SkillType::Skill => "skills",
            SkillType::Agent => "agents",
            SkillType::Command => "commands",
            SkillType::Rule => "rules",
        }
    }

    /// Alternative directory names for the resources format
    pub fn alt_dir_names(&self) -> &'static [&'static str] {
        match self {
            SkillType::Rule => &["cursor-rules"],
            _ => &[],
        }
    }
}

/// Metadata from meta.yaml files (resources format)
#[derive(Debug, Deserialize)]
pub struct ResourceMeta {
    pub name: String,
    #[allow(dead_code)]
    pub author: Option<String>,
    #[allow(dead_code)]
    pub description: Option<String>,
}

/// A single skill/agent/command file
#[derive(Debug, Clone)]
pub struct SkillFile {
    /// Name without extension (e.g., "commit")
    pub name: String,
    /// Full path to the source file
    pub path: PathBuf,
    /// Type of skill
    pub skill_type: SkillType,
}

/// A bundle containing skills, agents, commands, and rules
#[derive(Debug, Clone)]
pub struct Bundle {
    /// Bundle name (e.g., "cl", "gastro")
    pub name: String,
    /// Path to the bundle directory
    #[allow(dead_code)]
    pub path: PathBuf,
    /// Skills in this bundle
    pub skills: Vec<SkillFile>,
    /// Agents in this bundle
    pub agents: Vec<SkillFile>,
    /// Commands in this bundle
    pub commands: Vec<SkillFile>,
    /// Rules in this bundle
    pub rules: Vec<SkillFile>,
}

impl Bundle {
    /// Create a new bundle by scanning a directory
    pub fn from_path(path: PathBuf) -> anyhow::Result<Self> {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| anyhow::anyhow!("Invalid bundle path"))?
            .to_string();

        let skills = Self::scan_type(&path, SkillType::Skill)?;
        let agents = Self::scan_type(&path, SkillType::Agent)?;
        let commands = Self::scan_type(&path, SkillType::Command)?;
        let rules = Self::scan_type(&path, SkillType::Rule)?;

        Ok(Bundle {
            name,
            path,
            skills,
            agents,
            commands,
            rules,
        })
    }

    /// Create a bundle from a resources-format directory (legacy - single bundle)
    /// Structure: resources/{skills,commands,agents,cursor-rules}/resource-name/{meta.yaml,*.md}
    pub fn from_resources_path(path: PathBuf, bundle_name: String) -> anyhow::Result<Self> {
        let resources_dir = path.join("resources");
        if !resources_dir.exists() {
            return Ok(Bundle {
                name: bundle_name,
                path,
                skills: vec![],
                agents: vec![],
                commands: vec![],
                rules: vec![],
            });
        }

        let skills = Self::scan_resources_type(&resources_dir, SkillType::Skill)?;
        let agents = Self::scan_resources_type(&resources_dir, SkillType::Agent)?;
        let commands = Self::scan_resources_type(&resources_dir, SkillType::Command)?;
        let rules = Self::scan_resources_type(&resources_dir, SkillType::Rule)?;

        Ok(Bundle {
            name: bundle_name,
            path,
            skills,
            agents,
            commands,
            rules,
        })
    }

    /// Create multiple bundles from a resources-format directory
    /// Each resource folder becomes its own bundle (for community repos)
    /// Structure: resources/{skills,commands,agents,cursor-rules}/resource-name/{meta.yaml,*.md}
    pub fn list_from_resources_path(path: PathBuf) -> anyhow::Result<Vec<Bundle>> {
        let resources_dir = path.join("resources");
        if !resources_dir.exists() {
            return Ok(vec![]);
        }

        let mut bundles: std::collections::HashMap<String, Bundle> =
            std::collections::HashMap::new();

        // Scan all resource types
        for skill_type in [
            SkillType::Skill,
            SkillType::Agent,
            SkillType::Command,
            SkillType::Rule,
        ] {
            let mut dir_names = vec![skill_type.dir_name()];
            dir_names.extend(skill_type.alt_dir_names());

            for dir_name in dir_names {
                let type_dir = resources_dir.join(dir_name);
                if !type_dir.exists() {
                    continue;
                }

                for entry in std::fs::read_dir(&type_dir)? {
                    let entry = entry?;
                    let resource_dir = entry.path();

                    if !resource_dir.is_dir() {
                        continue;
                    }

                    let folder_name = resource_dir
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("");

                    // Skip templates and hidden
                    if folder_name.starts_with('.') || folder_name.starts_with('_') {
                        continue;
                    }

                    // Get or create bundle for this resource
                    if let Some(skill_file) =
                        Self::scan_resource_folder(&resource_dir, skill_type, folder_name)?
                    {
                        let bundle_name = skill_file.name.clone();
                        let bundle = bundles
                            .entry(bundle_name.clone())
                            .or_insert_with(|| Bundle {
                                name: bundle_name,
                                path: resource_dir.clone(),
                                skills: vec![],
                                agents: vec![],
                                commands: vec![],
                                rules: vec![],
                            });

                        match skill_type {
                            SkillType::Skill => bundle.skills.push(skill_file),
                            SkillType::Agent => bundle.agents.push(skill_file),
                            SkillType::Command => bundle.commands.push(skill_file),
                            SkillType::Rule => bundle.rules.push(skill_file),
                        }
                    }
                }
            }
        }

        let mut result: Vec<Bundle> = bundles.into_values().collect();
        result.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(result)
    }

    /// Check if a path uses the resources format
    pub fn is_resources_format(path: &PathBuf) -> bool {
        path.join("resources").is_dir()
    }

    /// Scan a subdirectory for .md files (original flat format)
    fn scan_type(bundle_path: &PathBuf, skill_type: SkillType) -> anyhow::Result<Vec<SkillFile>> {
        let type_dir = bundle_path.join(skill_type.dir_name());

        if !type_dir.exists() {
            return Ok(vec![]);
        }

        let mut files = vec![];

        for entry in std::fs::read_dir(&type_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_file() && path.extension().is_some_and(|e| e == "md") {
                let name = path
                    .file_stem()
                    .and_then(|n| n.to_str())
                    .ok_or_else(|| anyhow::anyhow!("Invalid file name"))?
                    .to_string();

                files.push(SkillFile {
                    name,
                    path,
                    skill_type,
                });
            }
        }

        // Sort for consistent output
        files.sort_by(|a, b| a.name.cmp(&b.name));

        Ok(files)
    }

    /// Scan a resources-format directory for skill folders
    /// Structure: resources/{type}/resource-name/{meta.yaml,*.md}
    fn scan_resources_type(
        resources_dir: &PathBuf,
        skill_type: SkillType,
    ) -> anyhow::Result<Vec<SkillFile>> {
        let mut files = vec![];

        // Try primary dir name and alternatives (e.g., "cursor-rules" for Rule)
        let mut dir_names = vec![skill_type.dir_name()];
        dir_names.extend(skill_type.alt_dir_names());

        for dir_name in dir_names {
            let type_dir = resources_dir.join(dir_name);

            if !type_dir.exists() {
                continue;
            }

            for entry in std::fs::read_dir(&type_dir)? {
                let entry = entry?;
                let resource_dir = entry.path();

                // Skip non-directories and hidden/template folders
                if !resource_dir.is_dir() {
                    continue;
                }

                let folder_name = resource_dir
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("");

                if folder_name.starts_with('.') || folder_name.starts_with('_') {
                    continue;
                }

                // Try to find the content file and extract name from meta.yaml
                if let Some(skill_file) =
                    Self::scan_resource_folder(&resource_dir, skill_type, folder_name)?
                {
                    files.push(skill_file);
                }
            }
        }

        // Sort for consistent output
        files.sort_by(|a, b| a.name.cmp(&b.name));

        Ok(files)
    }

    /// Scan a single resource folder for meta.yaml and content .md file
    fn scan_resource_folder(
        resource_dir: &PathBuf,
        skill_type: SkillType,
        folder_name: &str,
    ) -> anyhow::Result<Option<SkillFile>> {
        // Try to read meta.yaml to get the name
        let meta_path = resource_dir.join("meta.yaml");
        let name = if meta_path.exists() {
            match std::fs::read_to_string(&meta_path) {
                Ok(content) => match serde_yaml::from_str::<ResourceMeta>(&content) {
                    Ok(meta) => meta.name,
                    Err(_) => folder_name.to_string(),
                },
                Err(_) => folder_name.to_string(),
            }
        } else {
            folder_name.to_string()
        };

        // Find the content .md file (could be skill.md, command.md, agent.md, rule.md, or any .md)
        let expected_names = match skill_type {
            SkillType::Skill => vec!["skill.md", "SKILL.md"],
            SkillType::Agent => vec!["agent.md", "AGENT.md"],
            SkillType::Command => vec!["command.md", "COMMAND.md"],
            SkillType::Rule => vec!["rule.md", "RULE.md"],
        };

        // First try expected names
        for expected in &expected_names {
            let md_path = resource_dir.join(expected);
            if md_path.exists() {
                return Ok(Some(SkillFile {
                    name,
                    path: md_path,
                    skill_type,
                }));
            }
        }

        // Fall back to any .md file (excluding meta files)
        for entry in std::fs::read_dir(resource_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_file() && path.extension().is_some_and(|e| e == "md") {
                return Ok(Some(SkillFile {
                    name,
                    path,
                    skill_type,
                }));
            }
        }

        Ok(None)
    }

    /// Get all files of a specific type
    pub fn files_of_type(&self, skill_type: SkillType) -> &[SkillFile] {
        match skill_type {
            SkillType::Skill => &self.skills,
            SkillType::Agent => &self.agents,
            SkillType::Command => &self.commands,
            SkillType::Rule => &self.rules,
        }
    }

    /// Check if bundle is empty (no files)
    pub fn is_empty(&self) -> bool {
        self.skills.is_empty()
            && self.agents.is_empty()
            && self.commands.is_empty()
            && self.rules.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_skill_type_dir_name() {
        assert_eq!(SkillType::Skill.dir_name(), "skills");
        assert_eq!(SkillType::Agent.dir_name(), "agents");
        assert_eq!(SkillType::Command.dir_name(), "commands");
        assert_eq!(SkillType::Rule.dir_name(), "rules");
    }

    #[test]
    fn test_resources_format_detection() {
        let dir = tempdir().unwrap();

        // Without resources/ directory
        assert!(!Bundle::is_resources_format(&dir.path().to_path_buf()));

        // With resources/ directory
        fs::create_dir(dir.path().join("resources")).unwrap();
        assert!(Bundle::is_resources_format(&dir.path().to_path_buf()));
    }

    #[test]
    fn test_resources_format_bundle() {
        let dir = tempdir().unwrap();
        let resources = dir.path().join("resources");
        let skills_dir = resources.join("skills");
        let skill_folder = skills_dir.join("my-skill");

        fs::create_dir_all(&skill_folder).unwrap();

        // Create meta.yaml
        fs::write(
            skill_folder.join("meta.yaml"),
            "name: My Awesome Skill\nauthor: testuser\ndescription: A test skill\n",
        )
        .unwrap();

        // Create skill.md
        fs::write(skill_folder.join("skill.md"), "# My Skill\n\nContent here").unwrap();

        let bundle =
            Bundle::from_resources_path(dir.path().to_path_buf(), "test-repo".to_string()).unwrap();

        assert_eq!(bundle.name, "test-repo");
        assert_eq!(bundle.skills.len(), 1);
        assert_eq!(bundle.skills[0].name, "My Awesome Skill");
    }

    #[test]
    fn test_resources_format_cursor_rules() {
        let dir = tempdir().unwrap();
        let resources = dir.path().join("resources");
        let rules_dir = resources.join("cursor-rules");
        let rule_folder = rules_dir.join("my-rule");

        fs::create_dir_all(&rule_folder).unwrap();

        fs::write(
            rule_folder.join("meta.yaml"),
            "name: My Cursor Rule\nauthor: testuser\n",
        )
        .unwrap();

        fs::write(rule_folder.join("rule.md"), "# Rule content").unwrap();

        let bundle =
            Bundle::from_resources_path(dir.path().to_path_buf(), "test-repo".to_string()).unwrap();

        assert_eq!(bundle.rules.len(), 1);
        assert_eq!(bundle.rules[0].name, "My Cursor Rule");
    }

    #[test]
    fn test_resources_format_skips_templates() {
        let dir = tempdir().unwrap();
        let resources = dir.path().join("resources");
        let skills_dir = resources.join("skills");

        // Create template folder (should be skipped)
        let template = skills_dir.join("_example");
        fs::create_dir_all(&template).unwrap();
        fs::write(template.join("meta.yaml"), "name: Example\n").unwrap();
        fs::write(template.join("skill.md"), "# Example").unwrap();

        // Create real skill
        let skill = skills_dir.join("real-skill");
        fs::create_dir_all(&skill).unwrap();
        fs::write(skill.join("meta.yaml"), "name: Real Skill\n").unwrap();
        fs::write(skill.join("skill.md"), "# Real").unwrap();

        let bundle =
            Bundle::from_resources_path(dir.path().to_path_buf(), "test-repo".to_string()).unwrap();

        assert_eq!(bundle.skills.len(), 1);
        assert_eq!(bundle.skills[0].name, "Real Skill");
    }

    #[test]
    fn test_resources_format_fallback_to_folder_name() {
        let dir = tempdir().unwrap();
        let resources = dir.path().join("resources");
        let skills_dir = resources.join("skills");
        let skill_folder = skills_dir.join("my-skill");

        fs::create_dir_all(&skill_folder).unwrap();

        // No meta.yaml, should use folder name
        fs::write(skill_folder.join("skill.md"), "# Content").unwrap();

        let bundle =
            Bundle::from_resources_path(dir.path().to_path_buf(), "test-repo".to_string()).unwrap();

        assert_eq!(bundle.skills.len(), 1);
        assert_eq!(bundle.skills[0].name, "my-skill");
    }
}
