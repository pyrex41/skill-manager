use anyhow::Result;
use colored::Colorize;
use dialoguer::{theme::ColorfulTheme, Input, Select};

use crate::config::{Config, SourceConfig};

/// Run the first-time setup wizard
pub fn run_setup_wizard() -> Result<Config> {
    println!();
    println!(
        "{}",
        "No config found. Let's set up your skill sources.".bold()
    );
    println!();

    let options = vec![
        "Use ~/.claude-skills (recommended)",
        "Specify a custom path",
        "Skip for now (add sources later with `skm sources add`)",
    ];

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("How would you like to configure your default source?")
        .items(&options)
        .default(0)
        .interact()?;

    let sources = match selection {
        0 => {
            // Use default ~/.claude-skills
            let path = "~/.claude-skills".to_string();
            println!();
            println!("  {} {}", "Adding source:".dimmed(), path);
            vec![SourceConfig::Local { path, name: None }]
        }
        1 => {
            // Custom path
            let path: String = Input::with_theme(&ColorfulTheme::default())
                .with_prompt("Enter path to your skills directory")
                .interact_text()?;

            let path = if path.starts_with("~/") || path.starts_with('/') {
                path
            } else {
                // Make relative paths absolute
                let cwd = std::env::current_dir()?;
                cwd.join(&path).to_string_lossy().to_string()
            };

            println!();
            println!("  {} {}", "Adding source:".dimmed(), path);
            vec![SourceConfig::Local { path, name: None }]
        }
        2 => {
            // Skip
            println!();
            println!(
                "{}",
                "  Skipping source setup. Add sources later with:".dimmed()
            );
            println!("    skm sources add <path>");
            vec![]
        }
        _ => vec![],
    };

    let config = Config::new(sources);
    config.save()?;

    let config_path = Config::config_path()?;
    println!();
    println!("{} {}", "Config saved to:".green(), config_path.display());
    println!();

    Ok(config)
}
