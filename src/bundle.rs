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

    /// Scan a subdirectory for .md files
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

    #[test]
    fn test_skill_type_dir_name() {
        assert_eq!(SkillType::Skill.dir_name(), "skills");
        assert_eq!(SkillType::Agent.dir_name(), "agents");
        assert_eq!(SkillType::Command.dir_name(), "commands");
        assert_eq!(SkillType::Rule.dir_name(), "rules");
    }
}
