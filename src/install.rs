use anyhow::Result;
use colored::Colorize;
use std::path::PathBuf;

use crate::bundle::SkillType;
use crate::config::Config;
use crate::source::Source;
use crate::target::Tool;

/// Install a bundle to the target directory
pub fn install_bundle(
    config: &Config,
    bundle_name: &str,
    tool: &Tool,
    target_dir: &PathBuf,
    types: &[SkillType],
) -> Result<()> {
    // Find the bundle in configured sources
    let (_source, bundle) = config.find_bundle(bundle_name)?.ok_or_else(|| {
        // Collect available bundle names for the error message
        let mut available = vec![];
        for src in config.sources() {
            if let Ok(bundles) = src.list_bundles() {
                for b in bundles {
                    available.push(b.name);
                }
            }
        }
        anyhow::anyhow!(
            "Bundle not found: {}\nAvailable: {}",
            bundle_name,
            if available.is_empty() {
                "(none)".to_string()
            } else {
                available.join(", ")
            }
        )
    })?;

    println!(
        "Importing from {} to {}...",
        bundle_name.cyan(),
        tool.name()
    );

    let mut total_count = 0;

    for skill_type in types {
        let files = bundle.files_of_type(*skill_type);

        if files.is_empty() {
            continue;
        }

        let mut count = 0;

        for file in files {
            tool.write_file(target_dir, &bundle.name, file)?;
            count += 1;
        }

        if count > 0 {
            let dest_info = tool.dest_info(*skill_type, &bundle.name);
            println!(
                "  {}: {} files -> {}",
                skill_type.dir_name(),
                count,
                dest_info.dimmed()
            );
            total_count += count;
        }
    }

    if total_count == 0 {
        println!("{}", "No files to import.".yellow());
    } else {
        println!("{}", "Done!".green());
    }

    Ok(())
}

/// Install all bundles from a named source
pub fn install_from_source(
    source: &dyn Source,
    tool: &Tool,
    target_dir: &PathBuf,
    types: &[SkillType],
) -> Result<()> {
    let bundles = source.list_bundles()?;

    if bundles.is_empty() {
        println!("{}", "No bundles found in source.".yellow());
        return Ok(());
    }

    println!(
        "Installing {} bundle(s) from {} to {}...",
        bundles.len(),
        source.display_path().cyan(),
        tool.name()
    );
    println!();

    let mut total_files = 0;

    for bundle in bundles {
        let mut bundle_files = 0;

        for skill_type in types {
            let files = bundle.files_of_type(*skill_type);

            for file in files {
                tool.write_file(target_dir, &bundle.name, file)?;
                bundle_files += 1;
            }
        }

        if bundle_files > 0 {
            println!("  {} {} file(s)", bundle.name.cyan(), bundle_files);
            total_files += bundle_files;
        }
    }

    if total_files == 0 {
        println!("{}", "No files to import.".yellow());
    } else {
        println!();
        println!("{} {} file(s) installed.", "Done!".green(), total_files);
    }

    Ok(())
}

/// Install a specific bundle from a specific source
pub fn install_bundle_from_source(
    source: &dyn Source,
    bundle_name: &str,
    tool: &Tool,
    target_dir: &PathBuf,
    types: &[SkillType],
) -> Result<()> {
    let bundles = source.list_bundles()?;

    let bundle = bundles.into_iter().find(|b| b.name == bundle_name).ok_or_else(|| {
        anyhow::anyhow!(
            "Bundle '{}' not found in source '{}'",
            bundle_name,
            source.display_path()
        )
    })?;

    println!(
        "Importing from {} to {}...",
        bundle_name.cyan(),
        tool.name()
    );

    let mut total_count = 0;

    for skill_type in types {
        let files = bundle.files_of_type(*skill_type);

        if files.is_empty() {
            continue;
        }

        let mut count = 0;

        for file in files {
            tool.write_file(target_dir, &bundle.name, file)?;
            count += 1;
        }

        if count > 0 {
            let dest_info = tool.dest_info(*skill_type, &bundle.name);
            println!(
                "  {}: {} files -> {}",
                skill_type.dir_name(),
                count,
                dest_info.dimmed()
            );
            total_count += count;
        }
    }

    if total_count == 0 {
        println!("{}", "No files to import.".yellow());
    } else {
        println!("{}", "Done!".green());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn setup_test_source() -> (tempfile::TempDir, PathBuf) {
        let dir = tempdir().unwrap();
        let source_path = dir.path().to_path_buf();
        let bundle_dir = source_path.join("test-bundle");

        // Create commands
        let commands_dir = bundle_dir.join("commands");
        fs::create_dir_all(&commands_dir).unwrap();
        fs::write(commands_dir.join("commit.md"), "# Commit command").unwrap();
        fs::write(commands_dir.join("debug.md"), "# Debug command").unwrap();

        // Create agents
        let agents_dir = bundle_dir.join("agents");
        fs::create_dir_all(&agents_dir).unwrap();
        fs::write(agents_dir.join("analyzer.md"), "# Analyzer agent").unwrap();

        // Create skills
        let skills_dir = bundle_dir.join("skills");
        fs::create_dir_all(&skills_dir).unwrap();
        fs::write(skills_dir.join("helper.md"), "# Helper skill").unwrap();

        (dir, source_path)
    }

    #[test]
    fn test_install_to_claude() {
        let (_source_dir, source_path) = setup_test_source();
        let target_dir = tempdir().unwrap();

        // We can't easily test with Config since it's hardcoded for Phase 1
        // This test verifies the Tool::write_file logic directly

        let bundle = crate::bundle::Bundle::from_path(source_path.join("test-bundle")).unwrap();

        for cmd in &bundle.commands {
            Tool::Claude
                .write_file(&target_dir.path().to_path_buf(), "test-bundle", cmd)
                .unwrap();
        }

        // Verify files were created
        assert!(target_dir
            .path()
            .join(".claude/commands/test-bundle/commit.md")
            .exists());
        assert!(target_dir
            .path()
            .join(".claude/commands/test-bundle/debug.md")
            .exists());
    }

    #[test]
    fn test_install_to_opencode() {
        let (_source_dir, source_path) = setup_test_source();
        let target_dir = tempdir().unwrap();

        let bundle = crate::bundle::Bundle::from_path(source_path.join("test-bundle")).unwrap();

        // Test skill (should create directory structure)
        for skill in &bundle.skills {
            Tool::OpenCode
                .write_file(&target_dir.path().to_path_buf(), "test-bundle", skill)
                .unwrap();
        }

        // Verify skill structure
        assert!(target_dir
            .path()
            .join(".opencode/skill/test-bundle-helper/SKILL.md")
            .exists());

        // Test command
        for cmd in &bundle.commands {
            Tool::OpenCode
                .write_file(&target_dir.path().to_path_buf(), "test-bundle", cmd)
                .unwrap();
        }

        assert!(target_dir
            .path()
            .join(".opencode/command/test-bundle-commit.md")
            .exists());
    }

    #[test]
    fn test_install_to_cursor() {
        let (_source_dir, source_path) = setup_test_source();
        let target_dir = tempdir().unwrap();

        let bundle = crate::bundle::Bundle::from_path(source_path.join("test-bundle")).unwrap();

        // Test skill (should go to skills beta directory)
        for skill in &bundle.skills {
            Tool::Cursor
                .write_file(&target_dir.path().to_path_buf(), "test-bundle", skill)
                .unwrap();
        }

        // Verify skills folder-based structure (beta)
        assert!(target_dir
            .path()
            .join(".cursor/skills/test-bundle-helper/SKILL.md")
            .exists());

        // Test agent (should go to rules folder-based structure)
        for agent in &bundle.agents {
            Tool::Cursor
                .write_file(&target_dir.path().to_path_buf(), "test-bundle", agent)
                .unwrap();
        }

        // Verify rules folder-based structure
        assert!(target_dir
            .path()
            .join(".cursor/rules/test-bundle-analyzer/RULE.md")
            .exists());
    }
}
