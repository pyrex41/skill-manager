use serde::Deserialize;
use std::path::PathBuf;

use crate::bundle::{Bundle, BundleMeta, SkillFile, SkillType};

#[derive(Debug, Deserialize)]
pub struct SourceManifest {
    pub source: Option<SourceMeta>,
    #[serde(default)]
    pub bundles: Vec<BundleDeclaration>,
}

#[derive(Debug, Deserialize)]
pub struct SourceMeta {
    pub name: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct BundleDeclaration {
    pub name: String,
    pub path: String,
    pub description: Option<String>,
    pub tags: Option<Vec<String>>,
    #[serde(default)]
    pub paths: ComponentPaths,
}

#[derive(Debug, Deserialize, Default)]
pub struct ComponentPaths {
    pub skills: Option<String>,
    pub agents: Option<String>,
    pub commands: Option<String>,
    pub rules: Option<String>,
}

impl ComponentPaths {
    pub fn skills_dir(&self) -> &str {
        self.skills.as_deref().unwrap_or("skills")
    }
    pub fn agents_dir(&self) -> &str {
        self.agents.as_deref().unwrap_or("agents")
    }
    pub fn commands_dir(&self) -> &str {
        self.commands.as_deref().unwrap_or("commands")
    }
    pub fn rules_dir(&self) -> &str {
        self.rules.as_deref().unwrap_or("rules")
    }
}

/// Load and parse an skm.toml manifest from a source root directory
pub fn load_manifest(source_root: &PathBuf) -> Option<SourceManifest> {
    let manifest_path = source_root.join("skm.toml");
    if !manifest_path.exists() {
        return None;
    }
    let content = std::fs::read_to_string(&manifest_path).ok()?;
    toml::from_str(&content).ok()
}

/// Build a Bundle from a manifest declaration by scanning its declared paths
pub fn bundle_from_declaration(
    source_root: &PathBuf,
    decl: &BundleDeclaration,
) -> anyhow::Result<Bundle> {
    let bundle_root = source_root.join(&decl.path);

    let skills = scan_component_dir(
        &bundle_root.join(decl.paths.skills_dir()),
        SkillType::Skill,
    )?;
    let agents = scan_component_dir(
        &bundle_root.join(decl.paths.agents_dir()),
        SkillType::Agent,
    )?;
    let commands = scan_component_dir(
        &bundle_root.join(decl.paths.commands_dir()),
        SkillType::Command,
    )?;
    let rules = scan_component_dir(
        &bundle_root.join(decl.paths.rules_dir()),
        SkillType::Rule,
    )?;

    Ok(Bundle {
        name: decl.name.clone(),
        path: bundle_root,
        skills,
        agents,
        commands,
        rules,
        meta: BundleMeta {
            author: None,
            description: decl.description.clone(),
        },
    })
}

/// Scan a component directory for skill files.
/// Handles BOTH flat .md files AND {name}/SKILL.md directory format.
fn scan_component_dir(dir: &PathBuf, skill_type: SkillType) -> anyhow::Result<Vec<SkillFile>> {
    if !dir.exists() {
        return Ok(vec![]);
    }

    let mut files = vec![];

    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_file() && path.extension().is_some_and(|e| e == "md" || e == "mdc") {
            // Flat .md file (e.g., agents/base/review-agent.md)
            let name = path
                .file_stem()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();
            files.push(SkillFile {
                name,
                path,
                skill_type,
                source_dir: None,
            });
        } else if path.is_dir() {
            // Directory format: look for SKILL.md, AGENT.md, COMMAND.md, RULE.md, or any .md
            let expected_names = match skill_type {
                SkillType::Skill => vec!["SKILL.md", "skill.md"],
                SkillType::Agent => vec!["AGENT.md", "agent.md"],
                SkillType::Command => vec!["COMMAND.md", "command.md"],
                SkillType::Rule => vec!["RULE.md", "rule.md"],
            };

            let mut found = false;
            for expected in &expected_names {
                let md_path = path.join(expected);
                if md_path.exists() {
                    let folder_name = path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("")
                        .to_string();
                    files.push(SkillFile {
                        name: folder_name,
                        path: md_path,
                        skill_type,
                        source_dir: Some(path.clone()),
                    });
                    found = true;
                    break;
                }
            }

            // Fall back to any .md file in the directory
            if !found {
                if let Ok(entries) = std::fs::read_dir(&path) {
                    for sub_entry in entries.flatten() {
                        let sub_path = sub_entry.path();
                        if sub_path.is_file()
                            && sub_path.extension().is_some_and(|e| e == "md")
                        {
                            let folder_name = path
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or("")
                                .to_string();
                            files.push(SkillFile {
                                name: folder_name,
                                path: sub_path,
                                skill_type,
                                source_dir: Some(path.clone()),
                            });
                            break;
                        }
                    }
                }
            }
        }
    }

    files.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_load_manifest_not_present() {
        let dir = tempdir().unwrap();
        assert!(load_manifest(&dir.path().to_path_buf()).is_none());
    }

    #[test]
    fn test_load_manifest_minimal() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("skm.toml"),
            r#"
[[bundles]]
name = "my-bundle"
path = "src"
"#,
        )
        .unwrap();
        let manifest = load_manifest(&dir.path().to_path_buf()).unwrap();
        assert_eq!(manifest.bundles.len(), 1);
        assert_eq!(manifest.bundles[0].name, "my-bundle");
        assert!(manifest.source.is_none());
    }

    #[test]
    fn test_load_manifest_full() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("skm.toml"),
            r#"
[source]
name = "test-source"
description = "A test source"

[[bundles]]
name = "bundle-a"
path = "plugins/a"
description = "First bundle"
tags = ["test", "alpha"]

[bundles.paths]
skills = "skills/base"
agents = "agents/base"
commands = "commands/base"
rules = "rules/base"

[[bundles]]
name = "bundle-b"
path = "plugins/b"
"#,
        )
        .unwrap();
        let manifest = load_manifest(&dir.path().to_path_buf()).unwrap();
        assert_eq!(
            manifest.source.as_ref().unwrap().name.as_deref(),
            Some("test-source")
        );
        assert_eq!(manifest.bundles.len(), 2);
        assert_eq!(manifest.bundles[0].paths.skills_dir(), "skills/base");
        assert_eq!(manifest.bundles[1].paths.skills_dir(), "skills"); // default
    }

    #[test]
    fn test_component_paths_defaults() {
        let paths = ComponentPaths::default();
        assert_eq!(paths.skills_dir(), "skills");
        assert_eq!(paths.agents_dir(), "agents");
        assert_eq!(paths.commands_dir(), "commands");
        assert_eq!(paths.rules_dir(), "rules");
    }

    #[test]
    fn test_scan_component_dir_flat_files() {
        let dir = tempdir().unwrap();
        let agents_dir = dir.path().join("agents");
        fs::create_dir_all(&agents_dir).unwrap();
        fs::write(agents_dir.join("analyzer.md"), "# Analyzer").unwrap();
        fs::write(agents_dir.join("curator.md"), "# Curator").unwrap();

        let files = scan_component_dir(&agents_dir, SkillType::Agent).unwrap();
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].name, "analyzer");
        assert_eq!(files[1].name, "curator");
    }

    #[test]
    fn test_scan_component_dir_skill_md_directories() {
        let dir = tempdir().unwrap();
        let skills_dir = dir.path().join("skills");

        // Create SKILL.md directory format
        let skill1 = skills_dir.join("data-model-visualizer");
        fs::create_dir_all(&skill1).unwrap();
        fs::write(
            skill1.join("SKILL.md"),
            "---\nname: visualizer\n---\n# Viz",
        )
        .unwrap();

        let skill2 = skills_dir.join("system-mapper");
        fs::create_dir_all(&skill2).unwrap();
        fs::write(skill2.join("SKILL.md"), "# System Mapper").unwrap();

        let files = scan_component_dir(&skills_dir, SkillType::Skill).unwrap();
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].name, "data-model-visualizer");
        assert_eq!(files[1].name, "system-mapper");
    }

    #[test]
    fn test_scan_component_dir_mixed_formats() {
        let dir = tempdir().unwrap();
        let base = dir.path().join("base");

        // Mix of flat file and directory
        fs::create_dir_all(&base).unwrap();
        fs::write(base.join("simple.md"), "# Simple").unwrap();

        let skill_dir = base.join("complex-skill");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(skill_dir.join("SKILL.md"), "# Complex").unwrap();

        let files = scan_component_dir(&base, SkillType::Skill).unwrap();
        assert_eq!(files.len(), 2);
    }

    #[test]
    fn test_bundle_from_declaration() {
        let dir = tempdir().unwrap();
        let plugin = dir.path().join("plugins/docs");

        // Create skills/base with SKILL.md directory
        let skill_dir = plugin.join("skills/base/my-skill");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(skill_dir.join("SKILL.md"), "# Skill").unwrap();

        // Create agents/base with flat file
        let agents_dir = plugin.join("agents/base");
        fs::create_dir_all(&agents_dir).unwrap();
        fs::write(agents_dir.join("review-agent.md"), "# Agent").unwrap();

        let decl = BundleDeclaration {
            name: "synapse-docs".to_string(),
            path: "plugins/docs".to_string(),
            description: Some("Documentation plugin".to_string()),
            tags: None,
            paths: ComponentPaths {
                skills: Some("skills/base".to_string()),
                agents: Some("agents/base".to_string()),
                commands: Some("commands/base".to_string()),
                rules: Some("rules/base".to_string()),
            },
        };

        let bundle = bundle_from_declaration(&dir.path().to_path_buf(), &decl).unwrap();
        assert_eq!(bundle.name, "synapse-docs");
        assert_eq!(bundle.skills.len(), 1);
        assert_eq!(bundle.agents.len(), 1);
        assert_eq!(
            bundle.meta.description,
            Some("Documentation plugin".to_string())
        );
    }
}
