use anyhow::Result;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::bundle::{SkillFile, SkillType};

/// Target AI coding tool
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    Claude,
    OpenCode,
    Cursor,
    Codex,
}

/// Detected agent file format based on tools field syntax
#[derive(Debug, PartialEq)]
enum AgentFormat {
    /// Claude format: `tools: Read, Grep, Glob` (PascalCase, comma-separated)
    Claude,
    /// OpenCode format: `tools:\n  read: true` (lowercase, YAML object)
    OpenCode,
    /// No tools field found
    Unknown,
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
                std::env::current_dir().unwrap_or(home.clone())
            }
            Tool::Codex => home.join(".codex"),
        }
    }

    /// Get the name of this tool for display
    pub fn name(&self) -> &'static str {
        match self {
            Tool::Claude => "Claude",
            Tool::OpenCode => "OpenCode",
            Tool::Cursor => "Cursor",
            Tool::Codex => "Codex",
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
            Tool::Codex => self.write_codex(target_dir, bundle_name, skill),
        }
    }

    /// Get the destination info string for display
    pub fn dest_info(&self, skill_type: SkillType, bundle_name: &str) -> String {
        match self {
            Tool::Claude => match skill_type {
                SkillType::Skill => format!(".claude/skills/{}-*/SKILL.md", bundle_name),
                SkillType::Agent => format!(".claude/agents/{}/", bundle_name),
                SkillType::Command => format!(".claude/commands/{}/", bundle_name),
                SkillType::Rule => format!(".claude/rules/{}-*/RULE.md", bundle_name),
            },
            Tool::OpenCode => match skill_type {
                SkillType::Skill => format!(".opencode/skills/{}-*/", bundle_name),
                SkillType::Agent => ".opencode/agents/".to_string(),
                SkillType::Command => ".opencode/commands/".to_string(),
                SkillType::Rule => format!(".opencode/rules/{}-*/", bundle_name),
            },
            Tool::Cursor => match skill_type {
                SkillType::Skill => format!(".cursor/skills/{}-*/", bundle_name),
                SkillType::Agent => format!(".cursor/agents/{}-*.md", bundle_name),
                SkillType::Command => format!(".cursor/commands/{}-*.md", bundle_name),
                SkillType::Rule => format!(".cursor/rules/{}-*/", bundle_name),
            },
            Tool::Codex => match skill_type {
                SkillType::Skill => format!(".codex/skills/{}-*/SKILL.md", bundle_name),
                SkillType::Agent => format!(".codex/agents/{}-*.md", bundle_name),
                SkillType::Command => format!(".codex/commands/{}-*.md", bundle_name),
                SkillType::Rule => format!(".codex/rules/{}-*/RULE.md", bundle_name),
            },
        }
    }

    // Claude:
    //   skills -> .claude/skills/{bundle}-{name}/SKILL.md (folder-based with frontmatter)
    //   agents -> .claude/agents/{bundle}/{name}.md (flat file within bundle dir)
    //   commands -> .claude/commands/{bundle}/{name}.md (flat file within bundle dir)
    //   rules -> .claude/rules/{bundle}-{name}/RULE.md (folder-based)
    // Phase 1+4: detect agent format and reverse-transform if needed
    fn write_claude(
        &self,
        target_dir: &PathBuf,
        bundle_name: &str,
        skill: &SkillFile,
    ) -> Result<PathBuf> {
        match skill.skill_type {
            SkillType::Skill => {
                // Skills use folder-based format: .claude/skills/{bundle}-{name}/SKILL.md
                let combined_name = format!("{}-{}", bundle_name, skill.name);
                let dest_dir = target_dir.join(".claude/skills").join(&combined_name);
                fs::create_dir_all(&dest_dir)?;

                let dest_file = dest_dir.join("SKILL.md");
                transform_skill_file(&skill.path, &dest_file, &combined_name)?;

                copy_companion_files(skill, &dest_dir)?;

                Ok(dest_file)
            }
            SkillType::Rule => {
                // Rules use folder-based format: .claude/rules/{bundle}-{name}/RULE.md
                let combined_name = format!("{}-{}", bundle_name, skill.name);
                let dest_dir = target_dir.join(".claude/rules").join(&combined_name);
                fs::create_dir_all(&dest_dir)?;

                let dest_file = dest_dir.join("RULE.md");
                // Use skill transform to ensure frontmatter exists
                transform_skill_file(&skill.path, &dest_file, &combined_name)?;

                copy_companion_files(skill, &dest_dir)?;

                Ok(dest_file)
            }
            SkillType::Agent => {
                // Agents are flat files within bundle dir: .claude/agents/{bundle}/{name}.md
                let dest_dir = target_dir
                    .join(".claude/agents")
                    .join(bundle_name);
                fs::create_dir_all(&dest_dir)?;

                let dest_file = dest_dir.join(format!("{}.md", skill.name));

                match detect_agent_format(&skill.path)? {
                    AgentFormat::OpenCode => transform_agent_for_claude(&skill.path, &dest_file)?,
                    _ => { fs::copy(&skill.path, &dest_file)?; }
                }

                copy_companion_files(skill, &dest_dir)?;

                Ok(dest_file)
            }
            SkillType::Command => {
                // Commands are flat files within bundle dir: .claude/commands/{bundle}/{name}.md
                let dest_dir = target_dir
                    .join(".claude/commands")
                    .join(bundle_name);
                fs::create_dir_all(&dest_dir)?;

                let dest_file = dest_dir.join(format!("{}.md", skill.name));
                fs::copy(&skill.path, &dest_file)?;

                copy_companion_files(skill, &dest_dir)?;

                Ok(dest_file)
            }
        }
    }

    // OpenCode:
    //   skills -> .opencode/skills/{bundle}-{name}/SKILL.md (with frontmatter)
    //   agents -> .opencode/agents/{bundle}-{name}.md
    //   commands -> .opencode/commands/{bundle}-{name}.md
    // Phase 4: detect agent format before transforming
    fn write_opencode(
        &self,
        target_dir: &PathBuf,
        bundle_name: &str,
        skill: &SkillFile,
    ) -> Result<PathBuf> {
        let combined_name = format!("{}-{}", bundle_name, skill.name);

        match skill.skill_type {
            SkillType::Skill => {
                let dest_dir = target_dir.join(".opencode/skills").join(&combined_name);
                fs::create_dir_all(&dest_dir)?;

                let dest_file = dest_dir.join("SKILL.md");
                transform_skill_file(&skill.path, &dest_file, &combined_name)?;

                copy_companion_files(skill, &dest_dir)?;

                Ok(dest_file)
            }
            SkillType::Rule => {
                let dest_dir = target_dir.join(".opencode/rules").join(&combined_name);
                fs::create_dir_all(&dest_dir)?;

                let dest_file = dest_dir.join("RULE.md");
                transform_skill_file(&skill.path, &dest_file, &combined_name)?;

                copy_companion_files(skill, &dest_dir)?;

                Ok(dest_file)
            }
            SkillType::Agent => {
                // Flat file target — companion files not applicable
                let dest_dir = target_dir.join(".opencode/agents");
                fs::create_dir_all(&dest_dir)?;

                let dest_file = dest_dir.join(format!("{}.md", combined_name));

                match detect_agent_format(&skill.path)? {
                    AgentFormat::Claude => transform_agent_file(&skill.path, &dest_file)?,
                    _ => { fs::copy(&skill.path, &dest_file)?; }
                }

                Ok(dest_file)
            }
            SkillType::Command => {
                // Flat file target — companion files not applicable
                let dest_dir = target_dir.join(".opencode/commands");
                fs::create_dir_all(&dest_dir)?;

                let dest_file = dest_dir.join(format!("{}.md", combined_name));
                fs::copy(&skill.path, &dest_file)?;

                Ok(dest_file)
            }
        }
    }

    // Cursor:
    //   skills -> .cursor/skills/{bundle}-{name}/SKILL.md (folder-based with frontmatter)
    //   agents -> .cursor/agents/{bundle}-{name}.md (flat file, subagents)
    //   commands -> .cursor/commands/{bundle}-{name}.md (flat file)
    //   rules -> .cursor/rules/{bundle}-{name}/RULE.md (folder-based)
    fn write_cursor(
        &self,
        target_dir: &PathBuf,
        bundle_name: &str,
        skill: &SkillFile,
    ) -> Result<PathBuf> {
        let combined_name = format!("{}-{}", bundle_name, skill.name);

        match skill.skill_type {
            SkillType::Skill => {
                // Skills use .cursor/skills/ directory with SKILL.md
                let dest_dir = target_dir.join(".cursor/skills").join(&combined_name);
                fs::create_dir_all(&dest_dir)?;

                let dest_file = dest_dir.join("SKILL.md");
                transform_skill_file(&skill.path, &dest_file, &combined_name)?;

                copy_companion_files(skill, &dest_dir)?;

                Ok(dest_file)
            }
            SkillType::Agent => {
                // Agents (subagents) use .cursor/agents/ as flat files
                let dest_dir = target_dir.join(".cursor/agents");
                fs::create_dir_all(&dest_dir)?;

                let dest_file = dest_dir.join(format!("{}.md", combined_name));
                transform_cursor_agent(&skill.path, &dest_file, &combined_name)?;

                Ok(dest_file)
            }
            SkillType::Command => {
                // Commands use .cursor/commands/ as flat files
                let dest_dir = target_dir.join(".cursor/commands");
                fs::create_dir_all(&dest_dir)?;

                let dest_file = dest_dir.join(format!("{}.md", combined_name));
                fs::copy(&skill.path, &dest_file)?;

                Ok(dest_file)
            }
            SkillType::Rule => {
                // Rules use .cursor/rules/ with RULE.md (folder-based)
                let dest_dir = target_dir.join(".cursor/rules").join(&combined_name);
                fs::create_dir_all(&dest_dir)?;

                let dest_file = dest_dir.join("RULE.md");
                transform_cursor_rule(&skill.path, &dest_file, &combined_name)?;

                copy_companion_files(skill, &dest_dir)?;

                Ok(dest_file)
            }
        }
    }

    // Codex:
    //   skills -> .codex/skills/{bundle}-{name}/SKILL.md (folder-based with frontmatter)
    //   agents -> .codex/agents/{bundle}-{name}.md (flat file)
    //   commands -> .codex/commands/{bundle}-{name}.md (flat file)
    //   rules -> .codex/rules/{bundle}-{name}/RULE.md (folder-based)
    fn write_codex(
        &self,
        target_dir: &PathBuf,
        bundle_name: &str,
        skill: &SkillFile,
    ) -> Result<PathBuf> {
        let combined_name = format!("{}-{}", bundle_name, skill.name);

        match skill.skill_type {
            SkillType::Skill => {
                // Skills use .codex/skills/ directory with SKILL.md
                let dest_dir = target_dir.join(".codex/skills").join(&combined_name);
                fs::create_dir_all(&dest_dir)?;

                let dest_file = dest_dir.join("SKILL.md");
                transform_skill_file(&skill.path, &dest_file, &combined_name)?;

                copy_companion_files(skill, &dest_dir)?;

                Ok(dest_file)
            }
            SkillType::Agent => {
                // Agents use .codex/agents/ as flat files
                let dest_dir = target_dir.join(".codex/agents");
                fs::create_dir_all(&dest_dir)?;

                let dest_file = dest_dir.join(format!("{}.md", combined_name));
                // Codex uses similar format to Cursor for agents
                transform_cursor_agent(&skill.path, &dest_file, &combined_name)?;

                Ok(dest_file)
            }
            SkillType::Command => {
                // Commands use .codex/commands/ as flat files
                let dest_dir = target_dir.join(".codex/commands");
                fs::create_dir_all(&dest_dir)?;

                let dest_file = dest_dir.join(format!("{}.md", combined_name));
                fs::copy(&skill.path, &dest_file)?;

                Ok(dest_file)
            }
            SkillType::Rule => {
                // Rules use .codex/rules/ with RULE.md (folder-based)
                let dest_dir = target_dir.join(".codex/rules").join(&combined_name);
                fs::create_dir_all(&dest_dir)?;

                let dest_file = dest_dir.join("RULE.md");
                transform_cursor_rule(&skill.path, &dest_file, &combined_name)?;

                copy_companion_files(skill, &dest_dir)?;

                Ok(dest_file)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Phase 4: Agent format detection
// ---------------------------------------------------------------------------

/// Detect whether an agent file uses Claude format (PascalCase comma string)
/// or OpenCode format (lowercase YAML object)
fn detect_agent_format(src: &PathBuf) -> Result<AgentFormat> {
    let content = fs::read_to_string(src)?;
    let lines: Vec<&str> = content.lines().collect();

    let mut in_fm = false;
    for line in &lines {
        if *line == "---" {
            if in_fm { break; }
            in_fm = true;
            continue;
        }
        if in_fm && line.trim().starts_with("tools:") {
            if line.contains(",") {
                return Ok(AgentFormat::Claude); // "tools: Read, Grep, ..."
            } else {
                return Ok(AgentFormat::OpenCode); // "tools:" (YAML object follows)
            }
        }
    }
    Ok(AgentFormat::Unknown) // No tools field
}

// ---------------------------------------------------------------------------
// Phase 2: Skill file transformation with description injection
// ---------------------------------------------------------------------------

/// Transform a skill file to ensure it has proper frontmatter with name and description fields.
/// - Adds `name:` if missing
/// - Adds `description:` if missing (extracted from body content)
fn transform_skill_file(src: &PathBuf, dest: &PathBuf, skill_name: &str) -> Result<()> {
    let content = fs::read_to_string(src)?;
    let lines: Vec<&str> = content.lines().collect();

    let output = if lines.first() == Some(&"---") {
        // Has frontmatter - check what fields exist
        let mut in_frontmatter = false;
        let mut has_name = false;
        let mut has_description = false;
        let mut frontmatter_end = 0;

        for (i, line) in lines.iter().enumerate() {
            if *line == "---" {
                if in_frontmatter {
                    frontmatter_end = i;
                    break;
                }
                in_frontmatter = true;
                continue;
            }
            if in_frontmatter {
                if line.starts_with("name:") { has_name = true; }
                if line.starts_with("description:") { has_description = true; }
            }
        }

        if has_name && has_description {
            // Already has both required fields, use as-is
            content
        } else {
            let mut result = String::new();
            result.push_str("---\n");

            if !has_name {
                result.push_str(&format!("name: {}\n", skill_name));
            }

            // Copy existing frontmatter lines (between first --- and closing ---)
            for line in lines.iter().skip(1).take(frontmatter_end - 1) {
                result.push_str(line);
                result.push('\n');
            }

            if !has_description {
                let desc = extract_description_from_body(&lines, frontmatter_end + 1);
                result.push_str(&format!("description: \"{}\"\n", desc));
            }

            // Add closing --- and body
            for line in lines.iter().skip(frontmatter_end) {
                result.push_str(line);
                result.push('\n');
            }
            result
        }
    } else {
        // No frontmatter - add it with both name and description
        let desc = extract_description_from_body(&lines, 0);
        let mut result = String::new();
        result.push_str("---\n");
        result.push_str(&format!("name: {}\n", skill_name));
        result.push_str(&format!("description: \"{}\"\n", desc));
        result.push_str("---\n");
        result.push_str(&content);
        result
    };

    let mut file = fs::File::create(dest)?;
    file.write_all(output.as_bytes())?;

    Ok(())
}

/// Extract a description from the markdown body content.
/// Uses the first heading text or first non-empty paragraph.
fn extract_description_from_body(lines: &[&str], start_from: usize) -> String {
    for line in lines.iter().skip(start_from) {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed == "---" {
            continue;
        }
        if trimmed.starts_with('#') {
            // Use heading text as description
            let text = trimmed.trim_start_matches('#').trim();
            return truncate_description(text);
        }
        // Use first paragraph text
        return truncate_description(trimmed);
    }
    "Skill instructions".to_string()
}

/// Truncate a description to 200 characters max
fn truncate_description(text: &str) -> String {
    if text.len() <= 200 {
        text.to_string()
    } else {
        format!("{}...", &text[..197])
    }
}

// ---------------------------------------------------------------------------
// Phase 1: Agent file transformation (Claude → OpenCode)
// ---------------------------------------------------------------------------

/// Transform an agent file for OpenCode format, converting tools from string to YAML object.
/// Phase 1: expanded tool name mapping with pass-through for unknown tools.
fn transform_agent_file(src: &PathBuf, dest: &PathBuf) -> Result<()> {
    let content = fs::read_to_string(src)?;
    let lines: Vec<&str> = content.lines().collect();

    if lines.first() != Some(&"---") {
        // No frontmatter, just copy as-is
        fs::copy(src, dest)?;
        return Ok(());
    }

    // Parse frontmatter and transform
    let mut result = String::new();
    let mut in_frontmatter = false;
    let mut frontmatter_lines = Vec::new();
    let mut body_lines = Vec::new();
    let mut found_end = false;

    for line in &lines {
        if *line == "---" {
            if in_frontmatter {
                found_end = true;
                in_frontmatter = false;
                continue;
            } else {
                in_frontmatter = true;
                continue;
            }
        }

        if in_frontmatter && !found_end {
            frontmatter_lines.push(*line);
        } else {
            body_lines.push(*line);
        }
    }

    // Transform frontmatter
    result.push_str("---\n");

    let mut i = 0;
    while i < frontmatter_lines.len() {
        let line = frontmatter_lines[i];

        if line.trim().starts_with("tools:") && line.contains(",") {
            // Found tools string (Claude format), convert to YAML object
            let tools_str = line.trim_start_matches("tools:").trim();
            let tool_list: Vec<&str> = tools_str.split(',').map(|s| s.trim()).collect();

            result.push_str("tools:\n");

            for tool in tool_list {
                let opencode_tool = claude_to_opencode_tool(tool.trim());
                result.push_str(&format!("  {}: true\n", opencode_tool));
            }
        } else if line.trim().starts_with("color:") {
            // Remove invalid color field (not supported by OpenCode)
            i += 1;
            continue;
        } else {
            // Keep other fields
            result.push_str(line);
            result.push('\n');
        }

        i += 1;
    }

    result.push_str("---\n");

    // Add body
    for line in body_lines {
        result.push_str(line);
        result.push('\n');
    }

    let mut file = fs::File::create(dest)?;
    file.write_all(result.as_bytes())?;

    Ok(())
}

/// Map a Claude tool name to its OpenCode equivalent.
/// Unknown tools pass through as lowercase instead of being dropped.
fn claude_to_opencode_tool(tool: &str) -> &str {
    match tool {
        // Direct equivalents (both directions)
        "Read" | "read" => "read",
        "Write" | "write" => "write",
        "Edit" | "edit" => "edit",
        "Grep" | "grep" => "grep",
        "Glob" | "glob" => "glob",
        "Bash" | "bash" => "bash",
        "WebSearch" | "websearch" => "websearch",
        "WebFetch" | "webfetch" => "webfetch",
        "TodoWrite" | "todowrite" => "todowrite",
        "TodoRead" | "todoread" => "todoread",
        // Claude-specific → closest OpenCode equivalent
        "LS" => "bash",
        "MultiEdit" => "edit",
        "Task" => "bash",
        "NotebookEdit" => "edit",
        "NotebookRead" => "read",
        "AskUserQuestion" | "question" => "question",
        "KillBash" | "BashOutput" => "bash",
        // OpenCode-native tools (pass through)
        "list" => "list",
        "lsp" => "lsp",
        "patch" => "patch",
        "skill" => "skill",
        // Unknown: pass through as-is (don't drop)
        other => {
            eprintln!("Warning: Unknown tool '{}', passing through as-is", other);
            other
        }
    }
}

// ---------------------------------------------------------------------------
// Phase 1: Reverse agent transform (OpenCode → Claude)
// ---------------------------------------------------------------------------

/// Transform an agent file for Claude format.
/// Converts OpenCode YAML object tools back to Claude comma-separated PascalCase string.
fn transform_agent_for_claude(src: &PathBuf, dest: &PathBuf) -> Result<()> {
    let content = fs::read_to_string(src)?;
    let lines: Vec<&str> = content.lines().collect();

    if lines.first() != Some(&"---") {
        fs::copy(src, dest)?;
        return Ok(());
    }

    // Parse frontmatter and body
    let mut in_frontmatter = false;
    let mut frontmatter_lines = Vec::new();
    let mut body_lines = Vec::new();
    let mut found_end = false;

    for line in &lines {
        if *line == "---" {
            if in_frontmatter {
                found_end = true;
                in_frontmatter = false;
                continue;
            } else {
                in_frontmatter = true;
                continue;
            }
        }
        if in_frontmatter && !found_end {
            frontmatter_lines.push(*line);
        } else {
            body_lines.push(*line);
        }
    }

    let mut result = String::new();
    result.push_str("---\n");

    let mut i = 0;
    while i < frontmatter_lines.len() {
        let line = frontmatter_lines[i];

        if line.trim() == "tools:" {
            // YAML object format — collect tool entries and convert to comma string
            let mut tools = Vec::new();
            i += 1;
            while i < frontmatter_lines.len() {
                let inner = frontmatter_lines[i].trim();
                if inner.contains(": true") {
                    let tool_name = inner.split(':').next().unwrap_or("").trim();
                    let claude_tool = opencode_to_claude_tool(tool_name);
                    tools.push(claude_tool);
                    i += 1;
                } else if inner.contains(": false") {
                    // Skip disabled tools
                    i += 1;
                } else if inner.is_empty() || (!inner.starts_with(' ') && !inner.starts_with('-')) {
                    // No longer in tools block
                    break;
                } else {
                    i += 1;
                }
            }
            if !tools.is_empty() {
                result.push_str(&format!("tools: {}\n", tools.join(", ")));
            }
            continue; // don't increment i again
        } else if line.trim().starts_with("color:") {
            // Pass through color field (valid in Claude agents)
            result.push_str(line);
            result.push('\n');
        } else {
            result.push_str(line);
            result.push('\n');
        }

        i += 1;
    }

    result.push_str("---\n");

    for line in body_lines {
        result.push_str(line);
        result.push('\n');
    }

    let mut file = fs::File::create(dest)?;
    file.write_all(result.as_bytes())?;
    Ok(())
}

/// Map an OpenCode tool name to its Claude equivalent.
fn opencode_to_claude_tool(tool: &str) -> &str {
    match tool {
        "read" => "Read",
        "write" => "Write",
        "edit" => "Edit",
        "grep" => "Grep",
        "glob" => "Glob",
        "bash" => "Bash",
        "websearch" => "WebSearch",
        "webfetch" => "WebFetch",
        "todowrite" => "TodoWrite",
        "todoread" => "TodoRead",
        "question" => "AskUserQuestion",
        "list" => "LS",
        "lsp" => "lsp",
        "patch" => "patch",
        "skill" => "skill",
        // Unknown: pass through as-is
        other => other,
    }
}

// ---------------------------------------------------------------------------
// Phase 3: Cursor rule frontmatter enhancement
// ---------------------------------------------------------------------------

/// Transform a file into Cursor rule format with proper frontmatter.
/// Ensures description and alwaysApply fields are present so Cursor's
/// "Apply Intelligently" system can discover and use the rule.
fn transform_cursor_rule(src: &PathBuf, dest: &PathBuf, _skill_name: &str) -> Result<()> {
    let content = fs::read_to_string(src)?;
    let lines: Vec<&str> = content.lines().collect();

    let output = if lines.first() == Some(&"---") {
        // Has frontmatter — check what fields exist
        let mut has_description = false;
        let mut has_always_apply = false;
        let mut in_fm = false;
        let mut fm_end = 0;

        for (i, line) in lines.iter().enumerate() {
            if *line == "---" {
                if in_fm { fm_end = i; break; }
                in_fm = true;
                continue;
            }
            if in_fm {
                if line.starts_with("description:") { has_description = true; }
                if line.starts_with("alwaysApply:") { has_always_apply = true; }
            }
        }

        if has_description && has_always_apply {
            content
        } else {
            let mut result = String::new();
            result.push_str("---\n");

            // Copy existing frontmatter lines
            for line in lines.iter().skip(1).take(fm_end - 1) {
                result.push_str(line);
                result.push('\n');
            }

            if !has_description {
                let desc = extract_description_from_body(&lines, fm_end + 1);
                result.push_str(&format!("description: \"{}\"\n", desc));
            }
            if !has_always_apply {
                result.push_str("alwaysApply: false\n");
            }

            // Closing --- and body
            for line in lines.iter().skip(fm_end) {
                result.push_str(line);
                result.push('\n');
            }
            result
        }
    } else {
        // No frontmatter — create with Cursor rule fields
        let desc = extract_description_from_body(&lines, 0);
        let mut result = String::new();
        result.push_str("---\n");
        result.push_str(&format!("description: \"{}\"\n", desc));
        result.push_str("alwaysApply: false\n");
        result.push_str("---\n");
        result.push_str(&content);
        result
    };

    let mut file = fs::File::create(dest)?;
    file.write_all(output.as_bytes())?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Cursor agent (subagent) transformation
// ---------------------------------------------------------------------------

/// Transform an agent file for Cursor subagent format.
/// Cursor subagents use YAML frontmatter with name and description fields.
fn transform_cursor_agent(src: &PathBuf, dest: &PathBuf, skill_name: &str) -> Result<()> {
    let content = fs::read_to_string(src)?;
    let lines: Vec<&str> = content.lines().collect();

    let output = if lines.first() == Some(&"---") {
        // Has frontmatter — check what fields exist
        let mut has_name = false;
        let mut has_description = false;
        let mut in_fm = false;
        let mut fm_end = 0;

        for (i, line) in lines.iter().enumerate() {
            if *line == "---" {
                if in_fm { fm_end = i; break; }
                in_fm = true;
                continue;
            }
            if in_fm {
                if line.starts_with("name:") { has_name = true; }
                if line.starts_with("description:") { has_description = true; }
            }
        }

        if has_name && has_description {
            content
        } else {
            let mut result = String::new();
            result.push_str("---\n");

            if !has_name {
                result.push_str(&format!("name: {}\n", skill_name));
            }

            // Copy existing frontmatter lines (skip tools: field which isn't used by Cursor)
            for line in lines.iter().skip(1).take(fm_end - 1) {
                // Skip Claude-specific tools field
                if line.trim().starts_with("tools:") && line.contains(",") {
                    continue;
                }
                result.push_str(line);
                result.push('\n');
            }

            if !has_description {
                let desc = extract_description_from_body(&lines, fm_end + 1);
                result.push_str(&format!("description: \"{}\"\n", desc));
            }

            // Closing --- and body
            for line in lines.iter().skip(fm_end) {
                result.push_str(line);
                result.push('\n');
            }
            result
        }
    } else {
        // No frontmatter — create with Cursor subagent fields
        let desc = extract_description_from_body(&lines, 0);
        let mut result = String::new();
        result.push_str("---\n");
        result.push_str(&format!("name: {}\n", skill_name));
        result.push_str(&format!("description: \"{}\"\n", desc));
        result.push_str("---\n");
        result.push_str(&content);
        result
    };

    let mut file = fs::File::create(dest)?;
    file.write_all(output.as_bytes())?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Companion file copying
// ---------------------------------------------------------------------------

/// Copy companion files from source_dir to dest_dir, skipping the main .md file.
/// Companion files are scripts, templates, and other resources that live alongside
/// the main skill/rule markdown file in directory-based bundles.
fn copy_companion_files(skill: &SkillFile, dest_dir: &Path) -> Result<()> {
    let source_dir = match &skill.source_dir {
        Some(dir) => dir,
        None => return Ok(()),
    };

    let main_file = &skill.path;

    for entry in fs::read_dir(source_dir)? {
        let entry = entry?;
        let entry_path = entry.path();

        // Skip the main markdown file
        if entry_path == *main_file {
            continue;
        }

        let file_name = match entry.file_name().into_string() {
            Ok(name) => name,
            Err(_) => continue,
        };

        // Skip meta.yaml (resources format metadata, not a companion)
        if file_name == "meta.yaml" {
            continue;
        }

        let dest_path = dest_dir.join(&file_name);

        if entry_path.is_dir() {
            copy_dir_recursive(&entry_path, &dest_path)?;
        } else {
            fs::copy(&entry_path, &dest_path)?;
        }
    }

    Ok(())
}

/// Recursively copy a directory tree from src to dest.
fn copy_dir_recursive(src: &Path, dest: &Path) -> Result<()> {
    fs::create_dir_all(dest)?;

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let entry_path = entry.path();
        let dest_path = dest.join(entry.file_name());

        if entry_path.is_dir() {
            copy_dir_recursive(&entry_path, &dest_path)?;
        } else {
            fs::copy(&entry_path, &dest_path)?;
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bundle::{SkillFile, SkillType};
    use tempfile::tempdir;

    #[test]
    fn test_tool_names() {
        assert_eq!(Tool::Claude.name(), "Claude");
        assert_eq!(Tool::OpenCode.name(), "OpenCode");
        assert_eq!(Tool::Cursor.name(), "Cursor");
    }

    // ---- Phase 2: transform_skill_file with description injection ----

    #[test]
    fn test_transform_skill_no_frontmatter() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("src.md");
        let dest = dir.path().join("dest.md");

        fs::write(&src, "# My Skill\n\nContent here").unwrap();
        transform_skill_file(&src, &dest, "test-skill").unwrap();

        let result = fs::read_to_string(&dest).unwrap();
        assert!(result.contains("name: test-skill"));
        assert!(result.contains("description: \"My Skill\""));
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
    fn test_transform_skill_with_name_and_description() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("src.md");
        let dest = dir.path().join("dest.md");

        fs::write(&src, "---\nname: existing-name\ndescription: existing desc\n---\n# My Skill").unwrap();
        transform_skill_file(&src, &dest, "test-skill").unwrap();

        let result = fs::read_to_string(&dest).unwrap();
        assert!(result.contains("name: existing-name"));
        assert!(!result.contains("name: test-skill"));
        assert!(result.contains("description: existing desc"));
    }

    #[test]
    fn test_transform_skill_with_name_no_description() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("src.md");
        let dest = dir.path().join("dest.md");

        fs::write(&src, "---\nname: my-skill\n---\n# Great Skill\n\nDoes stuff").unwrap();
        transform_skill_file(&src, &dest, "test-skill").unwrap();

        let result = fs::read_to_string(&dest).unwrap();
        assert!(result.contains("name: my-skill"));
        assert!(result.contains("description: \"Great Skill\""));
    }

    #[test]
    fn test_transform_skill_empty_body_fallback_description() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("src.md");
        let dest = dir.path().join("dest.md");

        fs::write(&src, "").unwrap();
        transform_skill_file(&src, &dest, "test-skill").unwrap();

        let result = fs::read_to_string(&dest).unwrap();
        assert!(result.contains("name: test-skill"));
        assert!(result.contains("description: \"Skill instructions\""));
    }

    #[test]
    fn test_transform_skill_paragraph_as_description() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("src.md");
        let dest = dir.path().join("dest.md");

        fs::write(&src, "This is a paragraph description of the skill.\n\nMore content.").unwrap();
        transform_skill_file(&src, &dest, "test-skill").unwrap();

        let result = fs::read_to_string(&dest).unwrap();
        assert!(result.contains("description: \"This is a paragraph description of the skill.\""));
    }

    #[test]
    fn test_extract_description_truncation() {
        let long_text = "A".repeat(250);
        let result = truncate_description(&long_text);
        assert_eq!(result.len(), 200);
        assert!(result.ends_with("..."));
    }

    // ---- Phase 1: Tool name mapping (Claude → OpenCode) ----

    #[test]
    fn test_transform_agent_file_tools_conversion() {
        let temp_dir = tempdir().unwrap();
        let src_path = temp_dir.path().join("source.md");
        let dest_path = temp_dir.path().join("dest.md");

        let src_content = r#"---
name: test-agent
description: Test agent
tools: Read, Grep, Glob, LS
model: sonnet
color: yellow
---
This is the agent content.
"#;
        fs::write(&src_path, src_content).unwrap();
        transform_agent_file(&src_path, &dest_path).unwrap();

        let result = fs::read_to_string(&dest_path).unwrap();
        assert!(result.contains("name: test-agent"));
        assert!(result.contains("description: Test agent"));
        assert!(result.contains("tools:"));
        assert!(result.contains("  read: true"));
        assert!(result.contains("  grep: true"));
        assert!(result.contains("  glob: true"));
        assert!(result.contains("  bash: true"));
        assert!(result.contains("model: sonnet"));
        assert!(!result.contains("color: yellow"));
        assert!(result.contains("This is the agent content."));
    }

    #[test]
    fn test_transform_agent_expanded_tools() {
        let temp_dir = tempdir().unwrap();
        let src_path = temp_dir.path().join("source.md");
        let dest_path = temp_dir.path().join("dest.md");

        let src_content = "---\nname: full-agent\ntools: Write, Edit, Bash, Task, AskUserQuestion, MultiEdit, NotebookRead\n---\nContent\n";
        fs::write(&src_path, src_content).unwrap();
        transform_agent_file(&src_path, &dest_path).unwrap();

        let result = fs::read_to_string(&dest_path).unwrap();
        assert!(result.contains("  write: true"));
        assert!(result.contains("  edit: true"));
        assert!(result.contains("  bash: true"));
        assert!(result.contains("  question: true"));
        assert!(result.contains("  read: true")); // NotebookRead -> read
        // Task -> bash, MultiEdit -> edit (already present)
    }

    #[test]
    fn test_transform_agent_unknown_tool_passthrough() {
        let temp_dir = tempdir().unwrap();
        let src_path = temp_dir.path().join("source.md");
        let dest_path = temp_dir.path().join("dest.md");

        let src_content = "---\nname: mcp-agent\ntools: Read, CustomMCP, Grep\n---\nContent\n";
        fs::write(&src_path, src_content).unwrap();
        transform_agent_file(&src_path, &dest_path).unwrap();

        let result = fs::read_to_string(&dest_path).unwrap();
        assert!(result.contains("  read: true"));
        assert!(result.contains("  CustomMCP: true")); // passed through, not dropped
        assert!(result.contains("  grep: true"));
    }

    // ---- Phase 1: Reverse transform (OpenCode → Claude) ----

    #[test]
    fn test_transform_agent_for_claude() {
        let temp_dir = tempdir().unwrap();
        let src_path = temp_dir.path().join("source.md");
        let dest_path = temp_dir.path().join("dest.md");

        let src_content = "---\nname: oc-agent\ndescription: An OpenCode agent\ntools:\n  read: true\n  write: true\n  grep: true\nmodel: sonnet\n---\nAgent body.\n";
        fs::write(&src_path, src_content).unwrap();
        transform_agent_for_claude(&src_path, &dest_path).unwrap();

        let result = fs::read_to_string(&dest_path).unwrap();
        assert!(result.contains("tools: Read, Write, Grep"));
        assert!(result.contains("name: oc-agent"));
        assert!(result.contains("model: sonnet"));
        assert!(result.contains("Agent body."));
    }

    #[test]
    fn test_transform_agent_for_claude_skips_disabled() {
        let temp_dir = tempdir().unwrap();
        let src_path = temp_dir.path().join("source.md");
        let dest_path = temp_dir.path().join("dest.md");

        let src_content = "---\ntools:\n  read: true\n  write: false\n  bash: true\n---\nBody\n";
        fs::write(&src_path, src_content).unwrap();
        transform_agent_for_claude(&src_path, &dest_path).unwrap();

        let result = fs::read_to_string(&dest_path).unwrap();
        assert!(result.contains("tools: Read, Bash"));
        assert!(!result.contains("Write"));
    }

    // ---- Phase 4: Format detection ----

    #[test]
    fn test_detect_agent_format_claude() {
        let temp_dir = tempdir().unwrap();
        let src = temp_dir.path().join("agent.md");

        fs::write(&src, "---\ntools: Read, Grep, Glob\n---\nContent").unwrap();
        assert_eq!(detect_agent_format(&src).unwrap(), AgentFormat::Claude);
    }

    #[test]
    fn test_detect_agent_format_opencode() {
        let temp_dir = tempdir().unwrap();
        let src = temp_dir.path().join("agent.md");

        fs::write(&src, "---\ntools:\n  read: true\n  grep: true\n---\nContent").unwrap();
        assert_eq!(detect_agent_format(&src).unwrap(), AgentFormat::OpenCode);
    }

    #[test]
    fn test_detect_agent_format_unknown() {
        let temp_dir = tempdir().unwrap();
        let src = temp_dir.path().join("agent.md");

        fs::write(&src, "---\nname: no-tools\ndescription: test\n---\nContent").unwrap();
        assert_eq!(detect_agent_format(&src).unwrap(), AgentFormat::Unknown);
    }

    #[test]
    fn test_detect_agent_format_no_frontmatter() {
        let temp_dir = tempdir().unwrap();
        let src = temp_dir.path().join("agent.md");

        fs::write(&src, "# Just a markdown file\nNo frontmatter.").unwrap();
        assert_eq!(detect_agent_format(&src).unwrap(), AgentFormat::Unknown);
    }

    // ---- Phase 4: Write with auto-detection ----

    #[test]
    fn test_write_claude_from_opencode_agent() {
        let temp_dir = tempdir().unwrap();
        let target_dir = temp_dir.path().to_path_buf();

        // Source is OpenCode format
        let src_content = "---\nname: oc-agent\ntools:\n  read: true\n  write: true\n---\nBody\n";
        let src_path = temp_dir.path().join("source.md");
        fs::write(&src_path, src_content).unwrap();

        let skill = SkillFile {
            name: "oc-agent".to_string(),
            path: src_path,
            skill_type: SkillType::Agent,
            source_dir: None,
        };

        let result = Tool::Claude.write_file(&target_dir, "bundle", &skill).unwrap();
        let content = fs::read_to_string(&result).unwrap();

        // Should have been reverse-transformed to Claude format
        assert!(content.contains("tools: Read, Write"));
        assert!(!content.contains("  read: true"));
    }

    #[test]
    fn test_write_opencode_from_claude_agent() {
        let temp_dir = tempdir().unwrap();
        let target_dir = temp_dir.path().to_path_buf();

        // Source is Claude format
        let src_content = "---\nname: cl-agent\ntools: Read, Grep\n---\nBody\n";
        let src_path = temp_dir.path().join("source.md");
        fs::write(&src_path, src_content).unwrap();

        let skill = SkillFile {
            name: "cl-agent".to_string(),
            path: src_path,
            skill_type: SkillType::Agent,
            source_dir: None,
        };

        let result = Tool::OpenCode.write_file(&target_dir, "bundle", &skill).unwrap();
        let content = fs::read_to_string(&result).unwrap();

        // Should have been forward-transformed to OpenCode format
        assert!(content.contains("  read: true"));
        assert!(content.contains("  grep: true"));
        assert!(!content.contains("tools: Read"));
    }

    #[test]
    fn test_write_opencode_from_opencode_agent_no_double_transform() {
        let temp_dir = tempdir().unwrap();
        let target_dir = temp_dir.path().to_path_buf();

        // Source is already OpenCode format
        let src_content = "---\nname: oc-agent\ntools:\n  read: true\n  grep: true\n---\nBody\n";
        let src_path = temp_dir.path().join("source.md");
        fs::write(&src_path, src_content).unwrap();

        let skill = SkillFile {
            name: "oc-agent".to_string(),
            path: src_path,
            skill_type: SkillType::Agent,
            source_dir: None,
        };

        let result = Tool::OpenCode.write_file(&target_dir, "bundle", &skill).unwrap();
        let content = fs::read_to_string(&result).unwrap();

        // Should be copied as-is (no transform needed)
        assert!(content.contains("tools:"));
        assert!(content.contains("  read: true"));
        assert!(content.contains("  grep: true"));
    }

    // ---- Integration: write_file for skills ----

    #[test]
    fn test_write_opencode_skill() {
        let temp_dir = tempdir().unwrap();
        let target_dir = temp_dir.path().to_path_buf();

        let src_content = "# My Skill\n\nContent here";
        let src_path = temp_dir.path().join("source.md");
        fs::write(&src_path, src_content).unwrap();

        let skill = SkillFile {
            name: "my-skill".to_string(),
            path: src_path,
            skill_type: SkillType::Skill,
            source_dir: None,
        };

        let result = Tool::OpenCode.write_file(&target_dir, "test-bundle", &skill).unwrap();

        let expected_path = target_dir.join(".opencode/skills/test-bundle-my-skill/SKILL.md");
        assert_eq!(result, expected_path);
        assert!(expected_path.exists());

        let content = fs::read_to_string(&expected_path).unwrap();
        assert!(content.contains("name: test-bundle-my-skill"));
        assert!(content.contains("description: \"My Skill\""));
        assert!(content.contains("# My Skill"));
    }

    #[test]
    fn test_write_cursor_skill() {
        let temp_dir = tempdir().unwrap();
        let target_dir = temp_dir.path().to_path_buf();

        let src_content = "# My Skill\n\nContent here";
        let src_path = temp_dir.path().join("source.md");
        fs::write(&src_path, src_content).unwrap();

        let skill = SkillFile {
            name: "my-skill".to_string(),
            path: src_path,
            skill_type: SkillType::Skill,
            source_dir: None,
        };

        let result = Tool::Cursor.write_file(&target_dir, "test-bundle", &skill).unwrap();

        let expected_path = target_dir.join(".cursor/skills/test-bundle-my-skill/SKILL.md");
        assert_eq!(result, expected_path);
        assert!(expected_path.exists());

        let content = fs::read_to_string(&expected_path).unwrap();
        assert!(content.contains("name: test-bundle-my-skill"));
        assert!(content.contains("description: \"My Skill\""));
        assert!(content.contains("# My Skill"));
    }

    // ---- Phase 3: Cursor rule frontmatter ----

    #[test]
    fn test_write_cursor_rule() {
        let temp_dir = tempdir().unwrap();
        let target_dir = temp_dir.path().to_path_buf();

        let src_content = "# My Rule\n\nContent here";
        let src_path = temp_dir.path().join("source.md");
        fs::write(&src_path, src_content).unwrap();

        let skill = SkillFile {
            name: "my-rule".to_string(),
            path: src_path,
            skill_type: SkillType::Rule,
            source_dir: None,
        };

        let result = Tool::Cursor.write_file(&target_dir, "test-bundle", &skill).unwrap();

        let expected_path = target_dir.join(".cursor/rules/test-bundle-my-rule/RULE.md");
        assert_eq!(result, expected_path);
        assert!(expected_path.exists());

        let content = fs::read_to_string(&expected_path).unwrap();
        assert!(content.contains("description: \"My Rule\""));
        assert!(content.contains("alwaysApply: false"));
        assert!(content.contains("# My Rule"));
    }

    #[test]
    fn test_cursor_rule_with_existing_description() {
        let temp_dir = tempdir().unwrap();
        let src = temp_dir.path().join("src.md");
        let dest = temp_dir.path().join("dest.md");

        fs::write(&src, "---\ndescription: Existing desc\n---\n# Rule Content").unwrap();
        transform_cursor_rule(&src, &dest, "test-rule").unwrap();

        let result = fs::read_to_string(&dest).unwrap();
        assert!(result.contains("description: Existing desc"));
        assert!(result.contains("alwaysApply: false"));
        // Should NOT have a second description
        assert_eq!(result.matches("description:").count(), 1);
    }

    #[test]
    fn test_cursor_rule_with_both_fields() {
        let temp_dir = tempdir().unwrap();
        let src = temp_dir.path().join("src.md");
        let dest = temp_dir.path().join("dest.md");

        fs::write(&src, "---\ndescription: Complete rule\nalwaysApply: true\n---\n# Content").unwrap();
        transform_cursor_rule(&src, &dest, "test-rule").unwrap();

        let result = fs::read_to_string(&dest).unwrap();
        // Should be unchanged since both fields exist
        assert!(result.contains("description: Complete rule"));
        assert!(result.contains("alwaysApply: true"));
    }

    #[test]
    fn test_cursor_agent_goes_to_agents_dir() {
        let temp_dir = tempdir().unwrap();
        let target_dir = temp_dir.path().to_path_buf();

        // An agent file installed to Cursor goes to .cursor/agents/
        let src_content = "---\nname: my-agent\ntools: Read, Grep\n---\nAgent instructions.";
        let src_path = temp_dir.path().join("source.md");
        fs::write(&src_path, src_content).unwrap();

        let skill = SkillFile {
            name: "my-agent".to_string(),
            path: src_path,
            skill_type: SkillType::Agent,
            source_dir: None,
        };

        let result = Tool::Cursor.write_file(&target_dir, "tb", &skill).unwrap();
        
        // Should be in .cursor/agents/ as a flat file
        let expected_path = target_dir.join(".cursor/agents/tb-my-agent.md");
        assert_eq!(result, expected_path);
        assert!(expected_path.exists());

        let content = fs::read_to_string(&result).unwrap();
        // Should have name and description in frontmatter
        assert!(content.contains("name:"));
        assert!(content.contains("description:"));
        // Should NOT have tools field (Claude-specific)
        assert!(!content.contains("tools: Read"));
    }

    #[test]
    fn test_cursor_command_goes_to_commands_dir() {
        let temp_dir = tempdir().unwrap();
        let target_dir = temp_dir.path().to_path_buf();

        let src_content = "# My Command\n\nDo something useful.";
        let src_path = temp_dir.path().join("source.md");
        fs::write(&src_path, src_content).unwrap();

        let skill = SkillFile {
            name: "my-command".to_string(),
            path: src_path,
            skill_type: SkillType::Command,
            source_dir: None,
        };

        let result = Tool::Cursor.write_file(&target_dir, "tb", &skill).unwrap();
        
        // Should be in .cursor/commands/ as a flat file
        let expected_path = target_dir.join(".cursor/commands/tb-my-command.md");
        assert_eq!(result, expected_path);
        assert!(expected_path.exists());

        let content = fs::read_to_string(&expected_path).unwrap();
        assert!(content.contains("# My Command"));
    }

    // ---- Integration: write_file for agents ----

    #[test]
    fn test_write_opencode_agent() {
        let temp_dir = tempdir().unwrap();
        let target_dir = temp_dir.path().to_path_buf();

        let src_content = r#"---
name: test-agent
tools: Read, Grep
---
Agent content here.
"#;
        let src_path = temp_dir.path().join("source.md");
        fs::write(&src_path, src_content).unwrap();

        let skill = SkillFile {
            name: "test-agent".to_string(),
            path: src_path,
            skill_type: SkillType::Agent,
            source_dir: None,
        };

        let result = Tool::OpenCode.write_file(&target_dir, "test-bundle", &skill).unwrap();

        let expected_path = target_dir.join(".opencode/agents/test-bundle-test-agent.md");
        assert_eq!(result, expected_path);
        assert!(expected_path.exists());

        let content = fs::read_to_string(&expected_path).unwrap();
        assert!(content.contains("tools:"));
        assert!(content.contains("  read: true"));
        assert!(content.contains("  grep: true"));
        assert!(content.contains("Agent content here."));
    }

    // ---- Companion file copying ----

    #[test]
    fn test_companion_files_copied_claude() {
        let temp_dir = tempdir().unwrap();
        let target_dir = temp_dir.path().join("target");
        fs::create_dir_all(&target_dir).unwrap();

        // Set up a source directory with SKILL.md and companion files
        let source_dir = temp_dir.path().join("source/skills/pptx");
        fs::create_dir_all(&source_dir).unwrap();

        let skill_md = source_dir.join("SKILL.md");
        fs::write(&skill_md, "# PPTX Skill\n\nCreates presentations.").unwrap();

        // Companion files
        fs::write(source_dir.join("ooxml.md"), "# OOXML Reference").unwrap();
        let scripts_dir = source_dir.join("scripts");
        fs::create_dir_all(&scripts_dir).unwrap();
        fs::write(scripts_dir.join("build.sh"), "#!/bin/bash\necho hello").unwrap();

        // Nested subdir in scripts
        let nested = scripts_dir.join("lib");
        fs::create_dir_all(&nested).unwrap();
        fs::write(nested.join("helper.py"), "print('hi')").unwrap();

        let skill = SkillFile {
            name: "pptx".to_string(),
            path: skill_md,
            skill_type: SkillType::Skill,
            source_dir: Some(source_dir),
        };

        Tool::Claude.write_file(&target_dir, "my-bundle", &skill).unwrap();

        // Skills now go to .claude/skills/{bundle}-{name}/SKILL.md
        let dest_dir = target_dir.join(".claude/skills/my-bundle-pptx");
        // Main file should exist as SKILL.md
        assert!(dest_dir.join("SKILL.md").exists());
        // Companion .md file
        assert!(dest_dir.join("ooxml.md").exists());
        assert_eq!(
            fs::read_to_string(dest_dir.join("ooxml.md")).unwrap(),
            "# OOXML Reference"
        );
        // Script file in subdirectory
        assert!(dest_dir.join("scripts/build.sh").exists());
        // Nested file
        assert!(dest_dir.join("scripts/lib/helper.py").exists());
    }

    #[test]
    fn test_companion_files_copied_opencode_skill() {
        let temp_dir = tempdir().unwrap();
        let target_dir = temp_dir.path().join("target");
        fs::create_dir_all(&target_dir).unwrap();

        let source_dir = temp_dir.path().join("source/skills/pptx");
        fs::create_dir_all(&source_dir).unwrap();

        let skill_md = source_dir.join("SKILL.md");
        fs::write(&skill_md, "# PPTX Skill").unwrap();
        fs::write(source_dir.join("template.pptx"), "binary content").unwrap();

        let skill = SkillFile {
            name: "pptx".to_string(),
            path: skill_md,
            skill_type: SkillType::Skill,
            source_dir: Some(source_dir),
        };

        Tool::OpenCode.write_file(&target_dir, "bundle", &skill).unwrap();

        let dest_dir = target_dir.join(".opencode/skills/bundle-pptx");
        assert!(dest_dir.join("SKILL.md").exists());
        assert!(dest_dir.join("template.pptx").exists());
    }

    #[test]
    fn test_companion_files_copied_cursor_skill() {
        let temp_dir = tempdir().unwrap();
        let target_dir = temp_dir.path().join("target");
        fs::create_dir_all(&target_dir).unwrap();

        let source_dir = temp_dir.path().join("source/skills/pptx");
        fs::create_dir_all(&source_dir).unwrap();

        let skill_md = source_dir.join("SKILL.md");
        fs::write(&skill_md, "# PPTX Skill").unwrap();
        fs::write(source_dir.join("reference.md"), "# Ref").unwrap();

        let skill = SkillFile {
            name: "pptx".to_string(),
            path: skill_md,
            skill_type: SkillType::Skill,
            source_dir: Some(source_dir),
        };

        Tool::Cursor.write_file(&target_dir, "bundle", &skill).unwrap();

        let dest_dir = target_dir.join(".cursor/skills/bundle-pptx");
        assert!(dest_dir.join("SKILL.md").exists());
        assert!(dest_dir.join("reference.md").exists());
    }

    #[test]
    fn test_companion_files_skips_meta_yaml() {
        let temp_dir = tempdir().unwrap();
        let target_dir = temp_dir.path().join("target");
        fs::create_dir_all(&target_dir).unwrap();

        let source_dir = temp_dir.path().join("source/skills/pptx");
        fs::create_dir_all(&source_dir).unwrap();

        let skill_md = source_dir.join("SKILL.md");
        fs::write(&skill_md, "# PPTX Skill").unwrap();
        fs::write(source_dir.join("meta.yaml"), "name: pptx\nauthor: test").unwrap();
        fs::write(source_dir.join("helper.py"), "print('hi')").unwrap();

        let skill = SkillFile {
            name: "pptx".to_string(),
            path: skill_md,
            skill_type: SkillType::Skill,
            source_dir: Some(source_dir),
        };

        Tool::Claude.write_file(&target_dir, "bundle", &skill).unwrap();

        // Skills now go to .claude/skills/{bundle}-{name}/SKILL.md
        let dest_dir = target_dir.join(".claude/skills/bundle-pptx");
        assert!(dest_dir.join("SKILL.md").exists());
        assert!(dest_dir.join("helper.py").exists());
        // meta.yaml should NOT be copied
        assert!(!dest_dir.join("meta.yaml").exists());
    }

    #[test]
    fn test_no_companion_files_when_source_dir_none() {
        let temp_dir = tempdir().unwrap();
        let target_dir = temp_dir.path().to_path_buf();

        let src_path = temp_dir.path().join("source.md");
        fs::write(&src_path, "# Simple Skill").unwrap();

        let skill = SkillFile {
            name: "simple".to_string(),
            path: src_path,
            skill_type: SkillType::Skill,
            source_dir: None,
        };

        // Should succeed without errors even though source_dir is None
        let result = Tool::Claude.write_file(&target_dir, "bundle", &skill).unwrap();
        assert!(result.exists());

        // Verify it's in the correct location with SKILL.md filename
        let expected_path = target_dir.join(".claude/skills/bundle-simple/SKILL.md");
        assert_eq!(result, expected_path);
    }

    // ---- Claude skill folder-based format ----

    #[test]
    fn test_write_claude_skill() {
        let temp_dir = tempdir().unwrap();
        let target_dir = temp_dir.path().to_path_buf();

        let src_content = "# My Skill\n\nContent here";
        let src_path = temp_dir.path().join("source.md");
        fs::write(&src_path, src_content).unwrap();

        let skill = SkillFile {
            name: "my-skill".to_string(),
            path: src_path,
            skill_type: SkillType::Skill,
            source_dir: None,
        };

        let result = Tool::Claude.write_file(&target_dir, "test-bundle", &skill).unwrap();

        // Should be in folder-based format: .claude/skills/{bundle}-{name}/SKILL.md
        let expected_path = target_dir.join(".claude/skills/test-bundle-my-skill/SKILL.md");
        assert_eq!(result, expected_path);
        assert!(expected_path.exists());

        let content = fs::read_to_string(&expected_path).unwrap();
        // Should have frontmatter with name and description
        assert!(content.contains("name: test-bundle-my-skill"));
        assert!(content.contains("description: \"My Skill\""));
        assert!(content.contains("# My Skill"));
    }

    #[test]
    fn test_write_claude_rule() {
        let temp_dir = tempdir().unwrap();
        let target_dir = temp_dir.path().to_path_buf();

        let src_content = "# My Rule\n\nRule content here";
        let src_path = temp_dir.path().join("source.md");
        fs::write(&src_path, src_content).unwrap();

        let skill = SkillFile {
            name: "my-rule".to_string(),
            path: src_path,
            skill_type: SkillType::Rule,
            source_dir: None,
        };

        let result = Tool::Claude.write_file(&target_dir, "test-bundle", &skill).unwrap();

        // Should be in folder-based format: .claude/rules/{bundle}-{name}/RULE.md
        let expected_path = target_dir.join(".claude/rules/test-bundle-my-rule/RULE.md");
        assert_eq!(result, expected_path);
        assert!(expected_path.exists());
    }

    // ---- Codex support tests ----

    #[test]
    fn test_write_codex_skill() {
        let temp_dir = tempdir().unwrap();
        let target_dir = temp_dir.path().to_path_buf();

        let src_content = "# My Skill\n\nContent here";
        let src_path = temp_dir.path().join("source.md");
        fs::write(&src_path, src_content).unwrap();

        let skill = SkillFile {
            name: "my-skill".to_string(),
            path: src_path,
            skill_type: SkillType::Skill,
            source_dir: None,
        };

        let result = Tool::Codex.write_file(&target_dir, "test-bundle", &skill).unwrap();

        // Should be in folder-based format: .codex/skills/{bundle}-{name}/SKILL.md
        let expected_path = target_dir.join(".codex/skills/test-bundle-my-skill/SKILL.md");
        assert_eq!(result, expected_path);
        assert!(expected_path.exists());

        let content = fs::read_to_string(&expected_path).unwrap();
        assert!(content.contains("name: test-bundle-my-skill"));
        assert!(content.contains("description: \"My Skill\""));
    }

    #[test]
    fn test_write_codex_agent() {
        let temp_dir = tempdir().unwrap();
        let target_dir = temp_dir.path().to_path_buf();

        let src_content = "---\nname: my-agent\ntools: Read, Grep\n---\nAgent instructions.";
        let src_path = temp_dir.path().join("source.md");
        fs::write(&src_path, src_content).unwrap();

        let skill = SkillFile {
            name: "my-agent".to_string(),
            path: src_path,
            skill_type: SkillType::Agent,
            source_dir: None,
        };

        let result = Tool::Codex.write_file(&target_dir, "tb", &skill).unwrap();

        // Should be flat file: .codex/agents/{bundle}-{name}.md
        let expected_path = target_dir.join(".codex/agents/tb-my-agent.md");
        assert_eq!(result, expected_path);
        assert!(expected_path.exists());
    }

    #[test]
    fn test_write_codex_command() {
        let temp_dir = tempdir().unwrap();
        let target_dir = temp_dir.path().to_path_buf();

        let src_content = "# My Command\n\nDo something useful.";
        let src_path = temp_dir.path().join("source.md");
        fs::write(&src_path, src_content).unwrap();

        let skill = SkillFile {
            name: "my-command".to_string(),
            path: src_path,
            skill_type: SkillType::Command,
            source_dir: None,
        };

        let result = Tool::Codex.write_file(&target_dir, "tb", &skill).unwrap();

        // Should be flat file: .codex/commands/{bundle}-{name}.md
        let expected_path = target_dir.join(".codex/commands/tb-my-command.md");
        assert_eq!(result, expected_path);
        assert!(expected_path.exists());
    }

    #[test]
    fn test_write_codex_rule() {
        let temp_dir = tempdir().unwrap();
        let target_dir = temp_dir.path().to_path_buf();

        let src_content = "# My Rule\n\nRule content.";
        let src_path = temp_dir.path().join("source.md");
        fs::write(&src_path, src_content).unwrap();

        let skill = SkillFile {
            name: "my-rule".to_string(),
            path: src_path,
            skill_type: SkillType::Rule,
            source_dir: None,
        };

        let result = Tool::Codex.write_file(&target_dir, "test-bundle", &skill).unwrap();

        // Should be folder-based: .codex/rules/{bundle}-{name}/RULE.md
        let expected_path = target_dir.join(".codex/rules/test-bundle-my-rule/RULE.md");
        assert_eq!(result, expected_path);
        assert!(expected_path.exists());
    }
}
