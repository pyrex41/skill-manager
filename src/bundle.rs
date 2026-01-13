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

    /// Check if a path uses the Anthropic/marketplace format
    /// Structure: skills/{name}/SKILL.md at the root level
    pub fn is_anthropic_format(path: &PathBuf) -> bool {
        let skills_dir = path.join("skills");
        if !skills_dir.is_dir() {
            return false;
        }

        // Check if any subdirectory contains SKILL.md
        if let Ok(entries) = std::fs::read_dir(&skills_dir) {
            for entry in entries.flatten() {
                let subdir = entry.path();
                if subdir.is_dir() && subdir.join("SKILL.md").exists() {
                    return true;
                }
            }
        }
        false
    }

    /// Create multiple bundles from an Anthropic-format directory
    /// Each skill folder becomes its own bundle
    /// Structure: skills/{name}/SKILL.md (with optional YAML frontmatter)
    pub fn list_from_anthropic_path(path: PathBuf) -> anyhow::Result<Vec<Bundle>> {
        let skills_dir = path.join("skills");
        if !skills_dir.exists() {
            return Ok(vec![]);
        }

        let mut bundles = vec![];

        for entry in std::fs::read_dir(&skills_dir)? {
            let entry = entry?;
            let skill_dir = entry.path();

            if !skill_dir.is_dir() {
                continue;
            }

            let folder_name = skill_dir.file_name().and_then(|n| n.to_str()).unwrap_or("");

            // Skip hidden and template directories
            if folder_name.starts_with('.') || folder_name.starts_with('_') {
                continue;
            }

            let skill_md = skill_dir.join("SKILL.md");
            if !skill_md.exists() {
                continue;
            }

            // Extract name from YAML frontmatter if present, otherwise use folder name
            let name = Self::extract_frontmatter_name(&skill_md)
                .unwrap_or_else(|| folder_name.to_string());

            let skill_file = SkillFile {
                name: name.clone(),
                path: skill_md,
                skill_type: SkillType::Skill,
            };

            bundles.push(Bundle {
                name,
                path: skill_dir,
                skills: vec![skill_file],
                agents: vec![],
                commands: vec![],
                rules: vec![],
            });
        }

        bundles.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(bundles)
    }

    /// Extract the 'name' field from YAML frontmatter in a markdown file
    fn extract_frontmatter_name(path: &PathBuf) -> Option<String> {
        let content = std::fs::read_to_string(path).ok()?;
        if !content.starts_with("---") {
            return None;
        }

        // Find end of frontmatter
        let rest = &content[3..];
        let end_idx = rest.find("---")?;
        let frontmatter = &rest[..end_idx];

        // Parse as YAML and extract name
        #[derive(serde::Deserialize)]
        struct Frontmatter {
            name: Option<String>,
        }

        let fm: Frontmatter = serde_yaml::from_str(frontmatter).ok()?;
        fm.name
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

        let bundles = Bundle::list_from_resources_path(dir.path().to_path_buf()).unwrap();

        assert_eq!(bundles.len(), 1);
        assert_eq!(bundles[0].name, "My Awesome Skill");
        assert_eq!(bundles[0].skills.len(), 1);
        assert_eq!(bundles[0].skills[0].name, "My Awesome Skill");
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

        let bundles = Bundle::list_from_resources_path(dir.path().to_path_buf()).unwrap();

        assert_eq!(bundles.len(), 1);
        assert_eq!(bundles[0].name, "My Cursor Rule");
        assert_eq!(bundles[0].rules.len(), 1);
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

        let bundles = Bundle::list_from_resources_path(dir.path().to_path_buf()).unwrap();

        assert_eq!(bundles.len(), 1);
        assert_eq!(bundles[0].name, "Real Skill");
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

        let bundles = Bundle::list_from_resources_path(dir.path().to_path_buf()).unwrap();

        assert_eq!(bundles.len(), 1);
        assert_eq!(bundles[0].name, "my-skill");
    }

    // Anthropic format tests
    #[test]
    fn test_anthropic_format_detection() {
        let dir = tempdir().unwrap();

        // Without skills/ directory
        assert!(!Bundle::is_anthropic_format(&dir.path().to_path_buf()));

        // With skills/ directory but no SKILL.md
        fs::create_dir(dir.path().join("skills")).unwrap();
        assert!(!Bundle::is_anthropic_format(&dir.path().to_path_buf()));

        // With skills/{name}/SKILL.md
        let skill_dir = dir.path().join("skills").join("my-skill");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(skill_dir.join("SKILL.md"), "# Skill content").unwrap();
        assert!(Bundle::is_anthropic_format(&dir.path().to_path_buf()));
    }

    #[test]
    fn test_anthropic_format_with_frontmatter() {
        let dir = tempdir().unwrap();
        let skills_dir = dir.path().join("skills");
        let skill_dir = skills_dir.join("xlsx");

        fs::create_dir_all(&skill_dir).unwrap();

        // Create SKILL.md with YAML frontmatter (Anthropic style)
        fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: Excel Processor\ndescription: Process Excel files\n---\n\n# Excel Skill\n\nContent here",
        )
        .unwrap();

        let bundles = Bundle::list_from_anthropic_path(dir.path().to_path_buf()).unwrap();

        assert_eq!(bundles.len(), 1);
        assert_eq!(bundles[0].name, "Excel Processor");
        assert_eq!(bundles[0].skills.len(), 1);
        assert_eq!(bundles[0].skills[0].name, "Excel Processor");
    }

    #[test]
    fn test_anthropic_format_without_frontmatter() {
        let dir = tempdir().unwrap();
        let skills_dir = dir.path().join("skills");
        let skill_dir = skills_dir.join("my-skill");

        fs::create_dir_all(&skill_dir).unwrap();

        // Create SKILL.md without frontmatter
        fs::write(skill_dir.join("SKILL.md"), "# My Skill\n\nContent here").unwrap();

        let bundles = Bundle::list_from_anthropic_path(dir.path().to_path_buf()).unwrap();

        assert_eq!(bundles.len(), 1);
        assert_eq!(bundles[0].name, "my-skill"); // Falls back to folder name
        assert_eq!(bundles[0].skills.len(), 1);
    }

    #[test]
    fn test_anthropic_format_multiple_skills() {
        let dir = tempdir().unwrap();
        let skills_dir = dir.path().join("skills");

        // Create first skill
        let skill1 = skills_dir.join("pdf");
        fs::create_dir_all(&skill1).unwrap();
        fs::write(
            skill1.join("SKILL.md"),
            "---\nname: PDF Handler\n---\n\n# PDF Skill",
        )
        .unwrap();

        // Create second skill
        let skill2 = skills_dir.join("docx");
        fs::create_dir_all(&skill2).unwrap();
        fs::write(
            skill2.join("SKILL.md"),
            "---\nname: Word Handler\n---\n\n# Word Skill",
        )
        .unwrap();

        let bundles = Bundle::list_from_anthropic_path(dir.path().to_path_buf()).unwrap();

        assert_eq!(bundles.len(), 2);
        // Sorted alphabetically
        assert_eq!(bundles[0].name, "PDF Handler");
        assert_eq!(bundles[1].name, "Word Handler");
    }

    #[test]
    fn test_anthropic_format_skips_templates() {
        let dir = tempdir().unwrap();
        let skills_dir = dir.path().join("skills");

        // Create template folder (should be skipped)
        let template = skills_dir.join("_template");
        fs::create_dir_all(&template).unwrap();
        fs::write(template.join("SKILL.md"), "# Template").unwrap();

        // Create real skill
        let skill = skills_dir.join("real-skill");
        fs::create_dir_all(&skill).unwrap();
        fs::write(skill.join("SKILL.md"), "# Real Skill").unwrap();

        let bundles = Bundle::list_from_anthropic_path(dir.path().to_path_buf()).unwrap();

        assert_eq!(bundles.len(), 1);
        assert_eq!(bundles[0].name, "real-skill");
    }

    #[test]
    fn test_extract_frontmatter_name() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("test.md");

        // With frontmatter
        fs::write(
            &file,
            "---\nname: My Skill\ndescription: test\n---\n\n# Content",
        )
        .unwrap();
        assert_eq!(
            Bundle::extract_frontmatter_name(&file),
            Some("My Skill".to_string())
        );

        // Without frontmatter
        fs::write(&file, "# No Frontmatter").unwrap();
        assert_eq!(Bundle::extract_frontmatter_name(&file), None);

        // With frontmatter but no name field
        fs::write(&file, "---\ndescription: test\n---\n\n# Content").unwrap();
        assert_eq!(Bundle::extract_frontmatter_name(&file), None);
    }
}
