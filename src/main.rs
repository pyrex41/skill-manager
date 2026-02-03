mod bundle;
mod config;
mod discover;
mod install;
mod manifest;
mod setup;
mod source;
mod target;

use anyhow::Result;
use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::{generate, Shell};
use colored::Colorize;
use std::io;
use std::path::PathBuf;

use crate::bundle::SkillType;
use crate::config::{Config, SourceConfig};
use crate::install::{install_bundle, install_bundle_from_source, install_from_source};
use crate::setup::run_setup_wizard;
use crate::target::Tool;

#[derive(Parser)]
#[command(name = "skm")]
#[command(about = "Manage AI coding tool skills for Claude, OpenCode, Cursor, and Codex")]
#[command(version)]
#[command(args_conflicts_with_subcommands = true)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Bundle name to install (when no subcommand given)
    #[arg(value_name = "BUNDLE")]
    bundle: Option<String>,

    /// Install to OpenCode instead of Claude
    #[arg(short = 'o', long = "opencode", global = true)]
    opencode: bool,

    /// Install to Cursor instead of Claude
    #[arg(short = 'c', long = "cursor", global = true)]
    cursor: bool,

    /// Install to Codex instead of Claude
    #[arg(short = 'x', long = "codex", global = true)]
    codex: bool,

    /// Install globally (tool-specific location)
    #[arg(short = 'g', long = "global", global = true)]
    global: bool,

    /// Target directory (default: current directory)
    #[arg(short = 't', long = "to", global = true)]
    target: Option<PathBuf>,

    /// Filter: only install skills
    #[arg(long = "skills")]
    skills_only: bool,

    /// Filter: only install agents
    #[arg(long = "agents")]
    agents_only: bool,

    /// Filter: only install commands
    #[arg(long = "commands")]
    commands_only: bool,

    /// Filter: only install rules
    #[arg(long = "rules")]
    rules_only: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Install a bundle (alias for `skm <bundle>`)
    Add {
        /// Bundle name to install
        bundle: String,
    },
    /// Browse available bundles interactively
    List,
    /// Manage skill sources (interactive if no subcommand)
    Sources {
        #[command(subcommand)]
        action: Option<SourcesAction>,
    },
    /// Show installed skills in current directory
    Here {
        /// Filter by tool (claude, opencode, cursor)
        #[arg(long)]
        tool: Option<String>,

        /// Interactively remove skills
        #[arg(long)]
        remove: bool,

        /// Remove all installed skills
        #[arg(long)]
        clean: bool,

        /// Skip confirmation prompts
        #[arg(short = 'y', long)]
        yes: bool,
    },
    /// Update git sources and refresh installed skills
    Update {
        /// Only update git sources, don't refresh skills
        #[arg(long)]
        sources_only: bool,
    },
    /// Generate shell completions
    Completions {
        /// Shell to generate completions for
        #[arg(value_enum)]
        shell: Shell,
    },
    /// Convert between rule and command formats
    Convert {
        /// Source file to convert
        source: PathBuf,
        /// Convert to rule format (default: convert to command format)
        #[arg(long)]
        to_rule: bool,
        /// Output file (default: stdout)
        #[arg(long)]
        output: Option<PathBuf>,
    },
    /// Remove an installed bundle
    Rm {
        /// Bundle name to remove
        bundle: String,

        /// Skip confirmation prompt
        #[arg(short = 'y', long)]
        yes: bool,
    },
}

#[derive(Subcommand)]
enum SourcesAction {
    /// List configured sources
    List,
    /// Add a source (local path or git URL)
    Add {
        /// Path or URL to add
        path: String,
        /// Optional name for the source (e.g., "fg")
        #[arg(short = 'n', long = "name")]
        name: Option<String>,
    },
    /// Remove a source
    Remove {
        /// Path, URL, or name to remove
        path: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Check if this is first run (no config file) and we're not doing a specific subcommand
    let config = if !Config::exists()? && cli.command.is_none() && cli.bundle.is_none() {
        // First run - show setup wizard
        run_setup_wizard()?
    } else {
        // Load existing config or use defaults
        Config::load_or_default()?
    };

    // Determine target tool
    let tool = if cli.cursor {
        Tool::Cursor
    } else if cli.opencode {
        Tool::OpenCode
    } else if cli.codex {
        Tool::Codex
    } else {
        Tool::Claude
    };

    // Determine target directory
    let target_dir = if cli.global {
        tool.global_target()
    } else if let Some(t) = cli.target {
        t
    } else {
        std::env::current_dir()?
    };

    // Determine which types to install
    let types = if cli.skills_only || cli.agents_only || cli.commands_only || cli.rules_only {
        let mut t = vec![];
        if cli.skills_only {
            t.push(SkillType::Skill);
        }
        if cli.agents_only {
            t.push(SkillType::Agent);
        }
        if cli.commands_only {
            t.push(SkillType::Command);
        }
        if cli.rules_only {
            t.push(SkillType::Rule);
        }
        t
    } else {
        vec![
            SkillType::Skill,
            SkillType::Agent,
            SkillType::Command,
            SkillType::Rule,
        ]
    };

    match cli.command {
        Some(Commands::Add {
            bundle: bundle_name,
        }) => {
            // `skm add <bundle>` is an alias for `skm <bundle>`
            do_install(&config, &bundle_name, &tool, &target_dir, &types)?;
        }
        Some(Commands::List) => {
            browse_bundles(&config)?;
        }
        Some(Commands::Sources { action }) => match action {
            Some(SourcesAction::List) => {
                sources_list(&config)?;
            }
            Some(SourcesAction::Add { path, name }) => {
                sources_add(name, path)?;
            }
            Some(SourcesAction::Remove { path }) => {
                sources_remove(path)?;
            }
            None => {
                // Interactive sources management
                sources_interactive()?;
            }
        },
        Some(Commands::Here {
            tool: filter_tool,
            remove,
            clean,
            yes,
        }) => {
            if remove {
                interactive_remove(&target_dir, filter_tool.as_deref())?;
            } else if clean {
                clean_all_skills(&target_dir, filter_tool.as_deref(), yes)?;
            } else {
                show_installed_skills(&target_dir, filter_tool.as_deref())?;
            }
        }
        Some(Commands::Update { sources_only }) => {
            update_sources(&config)?;
            if !sources_only {
                refresh_installed_skills(&config, &tool, &target_dir, &types)?;
            }
        }
        Some(Commands::Completions { shell }) => {
            generate_completions(shell);
        }
        Some(Commands::Convert {
            source,
            to_rule,
            output,
        }) => {
            convert_format(&source, to_rule, output.as_ref())?;
        }
        Some(Commands::Rm { bundle, yes }) => {
            let filter_tool = if cli.cursor {
                Some("cursor")
            } else if cli.opencode {
                Some("opencode")
            } else {
                None
            };
            remove_bundle(&bundle, &target_dir, filter_tool, yes)?;
        }
        None => {
            // No subcommand - either list bundles or install a bundle
            if let Some(bundle_name) = cli.bundle {
                // Install the specified bundle
                do_install(&config, &bundle_name, &tool, &target_dir, &types)?;
            } else {
                // List available bundles
                list_bundles(&config)?;
            }
        }
    }

    Ok(())
}

fn browse_bundles(config: &Config) -> Result<()> {
    use crate::bundle::Bundle;
    use dialoguer::{theme::ColorfulTheme, FuzzySelect};

    let sources = config.sources();

    if sources.is_empty() {
        println!("{}", "No sources configured.".yellow());
        println!("Add a source with: skm sources add <path>");
        return Ok(());
    }

    // Collect all bundles with their source info
    let mut all_bundles: Vec<(String, Bundle)> = Vec::new();

    for source in &sources {
        match source.list_bundles() {
            Ok(bundles) => {
                for bundle in bundles {
                    all_bundles.push((source.display_path(), bundle));
                }
            }
            Err(e) => {
                eprintln!(
                    "  {} {} - {}",
                    "Warning:".yellow(),
                    source.display_path(),
                    e
                );
            }
        }
    }

    if all_bundles.is_empty() {
        println!("{}", "No bundles found in configured sources.".yellow());
        return Ok(());
    }

    loop {
        println!();
        println!("{}", "Available Bundles (type to search)".bold());
        println!();

        // Build display items with searchable content
        // Format: "name | description | author | counts | source"
        let items: Vec<String> = all_bundles
            .iter()
            .map(|(source, bundle)| {
                let desc = bundle
                    .meta
                    .description
                    .as_ref()
                    .map(|d| {
                        // Truncate long descriptions
                        if d.len() > 40 {
                            format!("{}...", &d[..37])
                        } else {
                            d.clone()
                        }
                    })
                    .unwrap_or_default();
                let author = bundle
                    .meta
                    .author
                    .as_ref()
                    .map(|a| format!("by {}", a))
                    .unwrap_or_default();
                let counts = format!(
                    "{}s {}a {}c",
                    bundle.skills.len(),
                    bundle.agents.len(),
                    bundle.commands.len()
                );
                // Include searchable content (name, author, description, skill names)
                let search_hint = bundle.search_string();
                if desc.is_empty() {
                    format!(
                        "{:<20} {:<15} {} {} [{}]",
                        bundle.name,
                        author.dimmed(),
                        counts.dimmed(),
                        format!("({})", source).dimmed(),
                        search_hint.dimmed()
                    )
                } else {
                    format!(
                        "{:<20} {} {:<15} {} {} [{}]",
                        bundle.name,
                        desc.dimmed(),
                        author.dimmed(),
                        counts.dimmed(),
                        format!("({})", source).dimmed(),
                        search_hint.dimmed()
                    )
                }
            })
            .collect();

        let sel = FuzzySelect::with_theme(&ColorfulTheme::default())
            .with_prompt("Select a bundle (type to filter, Esc to quit)")
            .items(&items)
            .default(0)
            .highlight_matches(true)
            .interact_opt()?;

        match sel {
            Some(idx) if idx < all_bundles.len() => {
                let (_, bundle) = &all_bundles[idx];
                show_bundle_details(bundle)?;
            }
            _ => break,
        }
    }

    Ok(())
}

fn show_bundle_details(bundle: &crate::bundle::Bundle) -> Result<()> {
    use dialoguer::{theme::ColorfulTheme, Select};

    loop {
        println!();
        println!("{} {}", "Bundle:".bold(), bundle.name.cyan());
        println!();

        let mut items: Vec<String> = Vec::new();
        let mut file_paths: Vec<Option<std::path::PathBuf>> = Vec::new();

        for (section, files) in [
            ("skills", &bundle.skills),
            ("agents", &bundle.agents),
            ("commands", &bundle.commands),
        ] {
            if !files.is_empty() {
                items.push(format!(
                    "── {}/{} ──",
                    section,
                    format!(" ({} files)", files.len()).dimmed()
                ));
                file_paths.push(None); // section header

                for file in files {
                    let preview = get_file_preview(&file.path);
                    items.push(format!("  {} {}", file.name, preview.dimmed()));
                    file_paths.push(Some(file.path.clone()));
                }
            }
        }

        items.push("← Back".to_string());
        file_paths.push(None);

        let sel = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("Select to view contents")
            .items(&items)
            .default(0)
            .interact()?;

        if sel >= items.len() - 1 {
            break;
        }

        let path = match &file_paths[sel] {
            Some(p) => p,
            None => continue, // section header
        };

        // Show file contents
        println!();
        println!("{}", "─".repeat(60).dimmed());
        if let Ok(content) = std::fs::read_to_string(path) {
            for line in content.lines().take(40) {
                println!("{}", line);
            }
            let line_count = content.lines().count();
            if line_count > 40 {
                println!(
                    "{}",
                    format!("... ({} more lines)", line_count - 40).dimmed()
                );
            }
        }
        println!("{}", "─".repeat(60).dimmed());
        println!();
    }

    Ok(())
}

fn get_file_preview(path: &std::path::PathBuf) -> String {
    if let Ok(content) = std::fs::read_to_string(path) {
        content
            .lines()
            .filter(|line| !line.trim().is_empty())
            .filter(|line| !line.starts_with("---"))
            .filter(|line| !line.contains(':') || line.starts_with('#'))
            .take(1)
            .map(|line| {
                let trimmed = line.trim_start_matches('#').trim();
                if trimmed.len() > 50 {
                    format!("- {}...", &trimmed[..47])
                } else {
                    format!("- {}", trimmed)
                }
            })
            .next()
            .unwrap_or_default()
    } else {
        String::new()
    }
}

fn sources_interactive() -> Result<()> {
    use dialoguer::{theme::ColorfulTheme, Input, Select};

    loop {
        let config = Config::load_or_default()?;
        let sources = config.source_configs();

        println!();
        println!("{}", "Skill Sources".bold());
        println!();

        if sources.is_empty() {
            println!("  {}", "(no sources configured)".dimmed());
        } else {
            for (i, source) in sources.iter().enumerate() {
                let type_label = match source {
                    SourceConfig::Local { .. } => "local",
                    SourceConfig::Git { .. } => "git",
                };
                let priority = format!("[{}]", i + 1).dimmed();
                let name_display = source
                    .name()
                    .map(|n| format!(" ({})", n.yellow()))
                    .unwrap_or_default();
                println!(
                    "  {} {}{} {}",
                    priority,
                    source.display().cyan(),
                    name_display,
                    format!("({})", type_label).dimmed()
                );
            }
        }
        println!();

        let mut options = vec!["Add source", "Remove source"];
        if sources.len() > 1 {
            options.push("Change priority");
        }
        options.push("Done");

        let selection = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("What would you like to do?")
            .items(&options)
            .default(options.len() - 1)
            .interact()?;

        match options[selection] {
            "Add source" => {
                let path: String = Input::with_theme(&ColorfulTheme::default())
                    .with_prompt("Enter path or git URL")
                    .interact_text()?;
                sources_add(None, path)?;
            }
            "Remove source" => {
                if sources.is_empty() {
                    println!("{}", "No sources to remove.".yellow());
                    continue;
                }
                let source_names: Vec<&str> = sources.iter().map(|s| s.display()).collect();
                let sel = Select::with_theme(&ColorfulTheme::default())
                    .with_prompt("Select source to remove")
                    .items(&source_names)
                    .interact()?;
                sources_remove(source_names[sel].to_string())?;
            }
            "Change priority" => {
                if sources.len() < 2 {
                    continue;
                }
                let source_names: Vec<String> = sources
                    .iter()
                    .enumerate()
                    .map(|(i, s)| format!("[{}] {}", i + 1, s.display()))
                    .collect();
                let sel = Select::with_theme(&ColorfulTheme::default())
                    .with_prompt("Select source to move")
                    .items(&source_names)
                    .interact()?;

                let positions: Vec<String> = (1..=sources.len())
                    .map(|i| format!("Position {}", i))
                    .collect();
                let new_pos = Select::with_theme(&ColorfulTheme::default())
                    .with_prompt("Move to position")
                    .items(&positions)
                    .default(sel)
                    .interact()?;

                if sel != new_pos {
                    let mut config = Config::load_or_default()?;
                    config.move_source(sel, new_pos)?;
                    config.save()?;
                    println!("{}", "Priority updated.".green());
                }
            }
            "Done" => break,
            _ => break,
        }
    }

    // Auto-update git sources on exit
    let config = Config::load_or_default()?;
    let git_sources = config.git_sources();
    if !git_sources.is_empty() {
        println!();
        println!("{}", "Updating git sources...".dimmed());
        for source in git_sources {
            match source.pull() {
                Ok(true) => {
                    println!("  {} {}", "Updated:".green(), source.url());
                }
                Ok(false) => {} // Already up to date, stay quiet
                Err(e) => {
                    println!("  {} {}: {}", "Error:".red(), source.url(), e);
                }
            }
        }
    }

    Ok(())
}

fn sources_list(config: &Config) -> Result<()> {
    println!("{}", "Configured sources:".bold());
    println!();

    let sources = config.source_configs();
    if sources.is_empty() {
        println!("  {}", "(none)".dimmed());
        println!();
        println!("Add a source with: skm sources add <path>");
    } else {
        for (i, source) in sources.iter().enumerate() {
            let type_label = match source {
                SourceConfig::Local { .. } => "local",
                SourceConfig::Git { .. } => "git",
            };
            let name_display = source
                .name()
                .map(|n| format!("[{}] ", n.cyan()))
                .unwrap_or_default();
            println!(
                "  {}. {}{} {}",
                i + 1,
                name_display,
                source.display(),
                format!("({})", type_label).dimmed()
            );
        }
    }
    println!();

    Ok(())
}

fn sources_add(name: Option<String>, path: String) -> Result<()> {
    let mut config = Config::load_or_default()?;

    // Determine if this is a git URL or local path
    let source =
        if path.starts_with("https://") || path.starts_with("git@") || path.ends_with(".git") {
            SourceConfig::Git {
                url: path.clone(),
                name,
            }
        } else {
            // Normalize local path
            let normalized = if path.starts_with("~/") || path.starts_with('/') {
                path.clone()
            } else {
                // Make relative path absolute
                let cwd = std::env::current_dir()?;
                cwd.join(&path).to_string_lossy().to_string()
            };
            SourceConfig::Local {
                path: normalized,
                name,
            }
        };

    // Check if path exists for local sources
    if let SourceConfig::Local { ref path, .. } = source {
        let expanded = if path.starts_with("~/") {
            let home = std::env::var("HOME")?;
            PathBuf::from(format!("{}/{}", home, &path[2..]))
        } else {
            PathBuf::from(path)
        };

        if !expanded.exists() {
            println!("{} Path does not exist: {}", "Warning:".yellow(), path);
        }
    }

    config.add_source(source);
    config.save()?;

    println!("{} {}", "Added source:".green(), path);

    Ok(())
}

fn sources_remove(path: String) -> Result<()> {
    let mut config = Config::load_or_default()?;

    if config.remove_source(&path) {
        config.save()?;
        println!("{} {}", "Removed source:".green(), path);
    } else {
        println!("{} Source not found: {}", "Error:".red(), path);
    }

    Ok(())
}

fn update_sources(config: &Config) -> Result<()> {
    let git_sources = config.git_sources();

    if git_sources.is_empty() {
        println!("{}", "No git sources configured.".yellow());
        println!("Add a git source with: skm sources add <git-url>");
        return Ok(());
    }

    println!("{}", "Updating git sources...".bold());
    println!();

    let mut updated = 0;
    let mut already_current = 0;
    let mut errors = 0;

    for source in git_sources {
        print!("  {} {}... ", "Updating".cyan(), source.url());

        match source.pull() {
            Ok(true) => {
                println!("{}", "updated".green());
                updated += 1;
            }
            Ok(false) => {
                println!("{}", "already up to date".dimmed());
                already_current += 1;
            }
            Err(e) => {
                println!("{}: {}", "error".red(), e);
                errors += 1;
            }
        }
    }

    println!();
    if updated > 0 {
        println!("  {} {} source(s) updated", "".green(), updated);
    }
    if already_current > 0 {
        println!(
            "  {} {} source(s) already up to date",
            "".dimmed(),
            already_current
        );
    }
    if errors > 0 {
        println!("  {} {} source(s) failed", "".red(), errors);
    }

    Ok(())
}

fn refresh_installed_skills(
    config: &Config,
    tool: &Tool,
    target_dir: &PathBuf,
    types: &[SkillType],
) -> Result<()> {
    use crate::discover::{discover_installed, filter_by_tool};
    use std::collections::HashSet;

    // Discover installed skills for this tool
    let tool_name = match tool {
        Tool::Claude => "claude",
        Tool::OpenCode => "opencode",
        Tool::Cursor => "cursor",
        Tool::Codex => "codex",
    };
    let skills = filter_by_tool(discover_installed(target_dir)?, tool_name);

    if skills.is_empty() {
        println!();
        println!("{}", "No installed skills to refresh.".yellow());
        return Ok(());
    }

    // Collect unique bundle names
    let mut bundles_to_refresh: HashSet<String> = HashSet::new();
    for skill in &skills {
        if let Some(ref bundle) = skill.bundle {
            bundles_to_refresh.insert(bundle.clone());
        } else {
            // For skills without bundle, use the skill name as bundle name
            bundles_to_refresh.insert(skill.name.clone());
        }
    }

    if bundles_to_refresh.is_empty() {
        println!();
        println!("{}", "No bundles to refresh.".yellow());
        return Ok(());
    }

    println!();
    println!("{}", "Refreshing installed skills...".bold());
    println!();

    let mut refreshed = 0;
    let mut not_found = 0;
    let mut errors = 0;

    for bundle_name in bundles_to_refresh {
        print!("  {} {}... ", "Refreshing".cyan(), bundle_name);

        // Try to find this bundle in sources
        match config.find_bundle(&bundle_name) {
            Ok(Some((_source, bundle))) => {
                // Re-install this bundle
                let mut count = 0;
                for skill_type in types {
                    let files = bundle.files_of_type(*skill_type);
                    for file in files {
                        match tool.write_file(target_dir, &bundle.name, file) {
                            Ok(_) => count += 1,
                            Err(e) => {
                                println!("{}: {}", "error".red(), e);
                                errors += 1;
                            }
                        }
                    }
                }
                if count > 0 {
                    println!("{} ({} files)", "done".green(), count);
                    refreshed += 1;
                } else {
                    println!("{}", "no files".dimmed());
                }
            }
            Ok(None) => {
                println!("{}", "not found in sources".yellow());
                not_found += 1;
            }
            Err(e) => {
                println!("{}: {}", "error".red(), e);
                errors += 1;
            }
        }
    }

    println!();
    if refreshed > 0 {
        println!("  {} {} bundle(s) refreshed", "✓".green(), refreshed);
    }
    if not_found > 0 {
        println!(
            "  {} {} bundle(s) not found in sources",
            "⚠".yellow(),
            not_found
        );
    }
    if errors > 0 {
        println!("  {} {} error(s)", "✗".red(), errors);
    }

    Ok(())
}

fn list_bundles(config: &Config) -> Result<()> {
    let sources = config.sources();

    if sources.is_empty() {
        println!("{}", "No sources configured.".yellow());
        println!("Add a source with: skm sources add <path>");
        return Ok(());
    }

    println!("{}", "Available bundles:".bold());
    println!();

    let mut found_any = false;
    let mut had_errors = false;

    for source in sources {
        // Handle source errors gracefully - warn and continue
        let bundles = match source.list_bundles() {
            Ok(b) => b,
            Err(e) => {
                eprintln!(
                    "  {} {} - {}",
                    "Warning:".yellow(),
                    source.display_path(),
                    e
                );
                had_errors = true;
                continue;
            }
        };

        if bundles.is_empty() {
            continue;
        }

        found_any = true;
        println!("  {} {}", "Source:".dimmed(), source.display_path());

        for bundle in bundles {
            // Show description on same line if available
            if let Some(desc) = &bundle.meta.description {
                println!("    {}/ - {}", bundle.name.cyan(), desc.dimmed());
            } else {
                println!("    {}/", bundle.name.cyan());
            }

            let skill_count = bundle.skills.len();
            let agent_count = bundle.agents.len();
            let command_count = bundle.commands.len();
            let rule_count = bundle.rules.len();

            if skill_count > 0 {
                println!("      {:<10} {} files", "skills/", skill_count);
            }
            if agent_count > 0 {
                println!("      {:<10} {} files", "agents/", agent_count);
            }
            if command_count > 0 {
                println!("      {:<10} {} files", "commands/", command_count);
            }
            if rule_count > 0 {
                println!("      {:<10} {} files", "rules/", rule_count);
            }
        }
        println!();
    }

    if !found_any {
        if had_errors {
            println!("  {}", "(no accessible bundles found)".dimmed());
        } else {
            println!("  {}", "(no bundles found in configured sources)".dimmed());
        }
        println!();
    }

    Ok(())
}

fn show_installed_skills(base: &PathBuf, filter_tool: Option<&str>) -> Result<()> {
    use crate::discover::{
        discover_installed, filter_by_tool, group_by_tool, InstalledTool, SkillType,
    };

    let mut skills = discover_installed(base)?;

    // Apply filter if provided
    if let Some(tool_filter) = filter_tool {
        skills = filter_by_tool(skills, tool_filter);
    }

    if skills.is_empty() {
        if filter_tool.is_some() {
            println!(
                "{}",
                "No installed skills found for the specified tool.".yellow()
            );
        } else {
            println!("{}", "No installed skills found.".yellow());
        }
        println!();
        println!("Install skills with: skm <bundle>");
        return Ok(());
    }

    println!("{}", "Installed skills:".bold());
    println!();

    let grouped = group_by_tool(&skills);

    // Define tool order
    let tool_order = [
        InstalledTool::Claude,
        InstalledTool::OpenCode,
        InstalledTool::Cursor,
        InstalledTool::Codex,
    ];

    for tool in &tool_order {
        if let Some(type_map) = grouped.get(tool) {
            println!("  {}", tool.display_name().cyan().bold());

            // Define type order
            let type_order = [SkillType::Skill, SkillType::Agent, SkillType::Command];

            for skill_type in &type_order {
                if let Some(skill_list) = type_map.get(skill_type) {
                    if !skill_list.is_empty() {
                        println!("    {}/", skill_type.plural().dimmed());

                        for skill in skill_list {
                            let display_name = if let Some(ref bundle) = skill.bundle {
                                format!("{}/{}", bundle, skill.name)
                            } else {
                                skill.name.clone()
                            };
                            println!("      {}", display_name);
                        }
                    }
                }
            }
            println!();
        }
    }

    // Show summary
    let total = skills.len();
    let by_tool: std::collections::HashMap<_, usize> =
        skills
            .iter()
            .fold(std::collections::HashMap::new(), |mut acc, s| {
                *acc.entry(s.tool).or_insert(0) += 1;
                acc
            });

    let summary_parts: Vec<String> = tool_order
        .iter()
        .filter_map(|t| {
            by_tool
                .get(t)
                .map(|count| format!("{} {}", count, t.display_name()))
        })
        .collect();

    println!(
        "  {} {} total ({})",
        "".dimmed(),
        total,
        summary_parts.join(", ")
    );
    println!();

    Ok(())
}

fn generate_completions(shell: Shell) {
    let mut cmd = Cli::command();
    generate(shell, &mut cmd, "skm", &mut io::stdout());
}

fn interactive_remove(base: &PathBuf, filter_tool: Option<&str>) -> Result<()> {
    use crate::discover::{discover_installed, filter_by_tool, group_same_skills, remove_skill};
    use dialoguer::{theme::ColorfulTheme, Confirm, MultiSelect};

    let mut skills = discover_installed(base)?;

    if let Some(tool_filter) = filter_tool {
        skills = filter_by_tool(skills, tool_filter);
    }

    if skills.is_empty() {
        println!("{}", "No installed skills found.".yellow());
        return Ok(());
    }

    // Group skills by unique ID (same skill across multiple tools)
    let grouped = group_same_skills(&skills);
    let mut skill_ids: Vec<_> = grouped.keys().cloned().collect();
    skill_ids.sort();

    // Build display items for multi-select
    let display_items: Vec<String> = skill_ids
        .iter()
        .map(|id| {
            let instances = grouped.get(id).unwrap();
            let tools: Vec<&str> = instances.iter().map(|s| s.tool.display_name()).collect();
            format!("{} ({})", id, tools.join(", "))
        })
        .collect();

    // Show multi-select
    println!("{}", "Select skills to remove:".bold());
    println!("{}", "(space to toggle, enter to confirm)".dimmed());
    println!();

    let selections = MultiSelect::with_theme(&ColorfulTheme::default())
        .items(&display_items)
        .interact()?;

    if selections.is_empty() {
        println!("{}", "No skills selected.".yellow());
        return Ok(());
    }

    // Collect skills to remove
    let mut to_remove: Vec<&crate::discover::InstalledSkill> = Vec::new();
    for idx in &selections {
        let id = &skill_ids[*idx];
        if let Some(instances) = grouped.get(id) {
            to_remove.extend(instances.iter().copied());
        }
    }

    // Build summary
    let summary: Vec<String> = selections
        .iter()
        .map(|idx| {
            let id = &skill_ids[*idx];
            let instances = grouped.get(id).unwrap();
            let tools: Vec<&str> = instances.iter().map(|s| s.tool.display_name()).collect();
            format!("  {} from {}", id.cyan(), tools.join(", "))
        })
        .collect();

    println!();
    println!("{}", "Will remove:".bold());
    for line in &summary {
        println!("{}", line);
    }
    println!();

    // Confirm
    let confirm = Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt(format!("Remove {} skill(s)?", to_remove.len()))
        .default(false)
        .interact()?;

    if !confirm {
        println!("{}", "Cancelled.".yellow());
        return Ok(());
    }

    // Remove the skills
    let mut removed = 0;
    let mut errors = 0;

    for skill in to_remove {
        match remove_skill(skill) {
            Ok(()) => {
                removed += 1;
            }
            Err(e) => {
                eprintln!(
                    "{}: Failed to remove {}: {}",
                    "Error".red(),
                    skill.path.display(),
                    e
                );
                errors += 1;
            }
        }
    }

    println!();
    if removed > 0 {
        println!("{} Removed {} skill(s)", "".green(), removed);
    }
    if errors > 0 {
        println!("{} Failed to remove {} skill(s)", "".red(), errors);
    }

    Ok(())
}

fn clean_all_skills(base: &PathBuf, filter_tool: Option<&str>, skip_confirm: bool) -> Result<()> {
    use crate::discover::{discover_installed, filter_by_tool, remove_skill};
    use dialoguer::{theme::ColorfulTheme, Confirm};

    let mut skills = discover_installed(base)?;

    if let Some(tool_filter) = filter_tool {
        skills = filter_by_tool(skills, tool_filter);
    }

    if skills.is_empty() {
        println!("{}", "No installed skills found.".yellow());
        return Ok(());
    }

    let count = skills.len();
    let tool_desc = filter_tool
        .map(|t| format!(" for {}", t))
        .unwrap_or_default();

    println!("{} {} skill(s){}", "Found".bold(), count, tool_desc);
    println!();

    // Confirm unless --yes flag
    let confirmed = if skip_confirm {
        true
    } else {
        Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt(format!("Remove all {} skill(s)?", count))
            .default(false)
            .interact()?
    };

    if !confirmed {
        println!("{}", "Cancelled.".yellow());
        return Ok(());
    }

    // Remove all skills
    let mut removed = 0;
    let mut errors = 0;

    for skill in &skills {
        match remove_skill(skill) {
            Ok(()) => {
                removed += 1;
            }
            Err(e) => {
                eprintln!(
                    "{}: Failed to remove {}: {}",
                    "Error".red(),
                    skill.path.display(),
                    e
                );
                errors += 1;
            }
        }
    }

    println!();
    if removed > 0 {
        println!("{} Removed {} skill(s)", "".green(), removed);
    }
    if errors > 0 {
        println!("{} Failed to remove {} skill(s)", "".red(), errors);
    }

    Ok(())
}

fn skill_matches_bundle(skill: &crate::discover::InstalledSkill, bundle_name: &str) -> bool {
    // Claude: bundle field is the actual bundle name
    if skill.bundle.as_deref() == Some(bundle_name) {
        return true;
    }
    // OpenCode/Cursor: combined name is "{bundle}-{name}"
    if skill.name.starts_with(&format!("{}-", bundle_name)) {
        return true;
    }
    // Exact name match (single-skill bundles where name == bundle)
    if skill.name == bundle_name {
        return true;
    }
    false
}

fn remove_bundle(
    bundle_name: &str,
    base: &PathBuf,
    filter_tool: Option<&str>,
    skip_confirm: bool,
) -> Result<()> {
    use crate::discover::{
        discover_installed, filter_by_tool, group_by_tool, remove_skill, InstalledTool, SkillType,
    };
    use dialoguer::{theme::ColorfulTheme, Confirm};

    let mut skills = discover_installed(base)?;

    if let Some(tool_filter) = filter_tool {
        skills = filter_by_tool(skills, tool_filter);
    }

    // Filter to skills belonging to this bundle
    skills.retain(|s| skill_matches_bundle(s, bundle_name));

    if skills.is_empty() {
        println!(
            "No installed skills found for bundle '{}'.",
            bundle_name.cyan()
        );
        return Ok(());
    }

    // Print what will be removed, grouped by tool
    println!("{}", "Will remove:".bold());
    println!();

    let grouped = group_by_tool(&skills);
    let tool_order = [
        InstalledTool::Claude,
        InstalledTool::OpenCode,
        InstalledTool::Cursor,
        InstalledTool::Codex,
    ];
    let type_order = [SkillType::Skill, SkillType::Agent, SkillType::Command, SkillType::Rule];

    for tool in &tool_order {
        if let Some(type_map) = grouped.get(tool) {
            println!("  {}", tool.display_name().cyan().bold());
            for skill_type in &type_order {
                if let Some(skill_list) = type_map.get(skill_type) {
                    for skill in skill_list {
                        println!(
                            "    {}/{} {}",
                            skill_type.plural().dimmed(),
                            skill.name,
                            format!("({})", skill.path.display()).dimmed()
                        );
                    }
                }
            }
        }
    }
    println!();

    // Confirm unless --yes
    let confirmed = if skip_confirm {
        true
    } else {
        Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt(format!(
                "Remove {} file(s) from bundle '{}'?",
                skills.len(),
                bundle_name
            ))
            .default(false)
            .interact()?
    };

    if !confirmed {
        println!("{}", "Cancelled.".yellow());
        return Ok(());
    }

    // Remove the skills
    let mut removed = 0;
    let mut errors = 0;

    for skill in &skills {
        match remove_skill(skill) {
            Ok(()) => {
                removed += 1;
            }
            Err(e) => {
                eprintln!(
                    "{}: Failed to remove {}: {}",
                    "Error".red(),
                    skill.path.display(),
                    e
                );
                errors += 1;
            }
        }
    }

    if removed > 0 {
        println!("{} Removed {} file(s)", "".green(), removed);
    }
    if errors > 0 {
        println!("{} Failed to remove {} file(s)", "".red(), errors);
    }

    Ok(())
}

fn convert_format(source: &PathBuf, to_rule: bool, output: Option<&PathBuf>) -> Result<()> {
    use std::fs;
    use std::io::Write;

    if !source.exists() {
        println!(
            "{} Source file does not exist: {}",
            "Error:".red(),
            source.display()
        );
        return Ok(());
    }

    let content = fs::read_to_string(source)?;
    let converted = if to_rule {
        convert_to_rule(&content, source)
    } else {
        convert_to_command(&content)
    };

    match output {
        Some(output_path) => {
            let mut file = fs::File::create(output_path)?;
            file.write_all(converted.as_bytes())?;
            println!(
                "{} Converted to {}",
                "Success:".green(),
                output_path.display()
            );
        }
        None => {
            println!("{}", converted);
        }
    }

    Ok(())
}

fn convert_to_rule(content: &str, source_path: &PathBuf) -> String {
    let lines: Vec<&str> = content.lines().collect();

    // Check if already has frontmatter
    if lines.first() == Some(&"---") {
        // Already has frontmatter, assume it is already in rule format
        return content.to_string();
    }

    // Extract title from filename or first heading
    let name = source_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("converted-rule");

    let title = if let Some(first_line) = lines.first() {
        if first_line.starts_with("#") {
            first_line.trim_start_matches("#").trim().to_string()
        } else {
            name.to_string()
        }
    } else {
        name.to_string()
    };

    // Create rule frontmatter
    let mut result = String::new();
    result.push_str("---\n");
    result.push_str(&format!("description: \"{}\"\n", title));
    result.push_str("alwaysApply: false\n");
    result.push_str("---\n");
    result.push('\n');
    result.push_str(content);

    result
}

fn convert_to_command(content: &str) -> String {
    let lines: Vec<&str> = content.lines().collect();

    // Check if it has frontmatter
    if lines.first() == Some(&"---") {
        // Find the end of frontmatter
        let mut in_frontmatter = false;
        let mut end_idx = 0;

        for (i, line) in lines.iter().enumerate() {
            if *line == "---" {
                if in_frontmatter {
                    end_idx = i + 1;
                    break;
                }
                in_frontmatter = true;
            }
        }

        // Skip frontmatter and return the rest
        if end_idx > 0 && end_idx < lines.len() {
            lines[end_idx..].join("\n").trim_start().to_string()
        } else {
            content.to_string()
        }
    } else {
        // No frontmatter, return as-is
        content.to_string()
    }
}

/// Parse a bundle reference that may be source-scoped.
/// "fg/synapse-docs" → (Some("fg"), Some("synapse-docs"))
/// "fg" → (None, Some("fg")) - could be source name OR bundle name
fn parse_bundle_ref(input: &str) -> (Option<&str>, Option<&str>) {
    if let Some((source, bundle)) = input.split_once('/') {
        (Some(source), Some(bundle))
    } else {
        (None, Some(input))
    }
}

/// Dispatch install command with support for source-scoped references
fn do_install(
    config: &Config,
    bundle_ref: &str,
    tool: &Tool,
    target_dir: &PathBuf,
    types: &[SkillType],
) -> Result<()> {
    let (source_name, bundle_name) = parse_bundle_ref(bundle_ref);

    match (source_name, bundle_name) {
        (Some(source_name), Some(bundle_name)) => {
            // Explicit source/bundle: "fg/synapse-docs"
            match config.find_source_by_name(source_name) {
                Some((source, _)) => {
                    install_bundle_from_source(source.as_ref(), bundle_name, tool, target_dir, types)
                }
                None => {
                    anyhow::bail!("Source '{}' not found. Add it with: skm sources add {} <path>", source_name, source_name);
                }
            }
        }
        (None, Some(name)) => {
            // Just a name - could be a source name or bundle name
            // First check if it's a named source
            if let Some((source, _)) = config.find_source_by_name(name) {
                // Install all bundles from this source
                return install_from_source(source.as_ref(), tool, target_dir, types);
            }

            // Otherwise, search all sources for a bundle with this name
            install_bundle(config, name, tool, target_dir, types)
        }
        (None, None) => {
            anyhow::bail!("No bundle specified");
        }
        (Some(_), None) => {
            anyhow::bail!("Invalid bundle reference");
        }
    }
}

#[cfg(test)]
mod convert_tests {
    use super::*;

    #[test]
    fn test_convert_to_rule_no_frontmatter() {
        let content = "# Test Rule\n\nSome content here";
        let path = PathBuf::from("test-rule.md");
        let result = convert_to_rule(content, &path);

        assert!(result.starts_with("---\n"));
        assert!(result.contains("description: \"Test Rule\""));
        assert!(result.contains("alwaysApply: false"));
        assert!(result.contains("# Test Rule"));
    }

    #[test]
    fn test_convert_to_rule_with_existing_frontmatter() {
        let content = "---\ndescription: existing\n---\n# Content";
        let path = PathBuf::from("test.md");
        let result = convert_to_rule(content, &path);

        // Should return unchanged since it already has frontmatter
        assert_eq!(result, content);
    }

    #[test]
    fn test_convert_to_rule_uses_filename_when_no_heading() {
        let content = "Some content without a heading";
        let path = PathBuf::from("my-custom-rule.md");
        let result = convert_to_rule(content, &path);

        assert!(result.contains("description: \"my-custom-rule\""));
    }

    #[test]
    fn test_convert_to_command_strips_frontmatter() {
        let content =
            "---\ndescription: test\nalwaysApply: false\n---\n# Rule Content\n\nBody here";
        let result = convert_to_command(content);

        assert!(!result.contains("---"));
        assert!(!result.contains("description:"));
        assert!(result.starts_with("# Rule Content"));
        assert!(result.contains("Body here"));
    }

    #[test]
    fn test_convert_to_command_no_frontmatter() {
        let content = "# Simple Content\n\nNo frontmatter here";
        let result = convert_to_command(content);

        // Should return unchanged
        assert_eq!(result, content);
    }

    #[test]
    fn test_convert_to_command_only_frontmatter() {
        let content = "---\ndescription: test\n---";
        let result = convert_to_command(content);

        // Edge case: only frontmatter, no content after
        assert_eq!(result, content);
    }
}
