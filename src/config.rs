use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::source::{GitSource, LocalSource, Source};

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub default_tool: String,

    #[serde(default)]
    sources: Vec<SourceConfig>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type")]
pub enum SourceConfig {
    #[serde(rename = "local")]
    Local {
        path: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        name: Option<String>,
    },
    #[serde(rename = "git")]
    Git {
        url: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        name: Option<String>,
    },
}

impl Config {
    /// Create a new config with the given sources
    pub fn new(sources: Vec<SourceConfig>) -> Self {
        Config {
            default_tool: "claude".to_string(),
            sources,
        }
    }

    /// Load config from file, return None if it doesn't exist
    pub fn load() -> Result<Option<Self>> {
        let config_path = Self::config_path()?;

        if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)?;
            let config: Config = toml::from_str(&content)?;
            Ok(Some(config))
        } else {
            Ok(None)
        }
    }

    /// Load config from file or return default with ~/.claude-skills as source
    pub fn load_or_default() -> Result<Self> {
        if let Some(config) = Self::load()? {
            Ok(config)
        } else {
            // Fallback default - used when no config exists and not in interactive mode
            Ok(Config {
                default_tool: "claude".to_string(),
                sources: vec![SourceConfig::Local {
                    path: "~/.claude-skills".to_string(),
                    name: None,
                }],
            })
        }
    }

    /// Save config to file
    pub fn save(&self) -> Result<()> {
        let config_path = Self::config_path()?;

        // Create parent directory if needed
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let content = toml::to_string_pretty(self)?;
        std::fs::write(&config_path, content)?;

        Ok(())
    }

    /// Get the config file path
    pub fn config_path() -> Result<PathBuf> {
        let proj_dirs = directories::ProjectDirs::from("", "", "skm")
            .ok_or_else(|| anyhow::anyhow!("Could not determine config directory"))?;
        Ok(proj_dirs.config_dir().join("config.toml"))
    }

    /// Check if config file exists
    pub fn exists() -> Result<bool> {
        let config_path = Self::config_path()?;
        Ok(config_path.exists())
    }

    /// Get all configured sources as Source trait objects
    pub fn sources(&self) -> Vec<Box<dyn Source>> {
        self.sources
            .iter()
            .filter_map(|s| match s {
                SourceConfig::Local { path, .. } => {
                    let expanded = expand_tilde(path);
                    Some(Box::new(LocalSource::new(expanded)) as Box<dyn Source>)
                }
                SourceConfig::Git { url, .. } => match GitSource::new(url.clone()) {
                    Ok(source) => Some(Box::new(source) as Box<dyn Source>),
                    Err(e) => {
                        eprintln!("Warning: Could not initialize git source {}: {}", url, e);
                        None
                    }
                },
            })
            .collect()
    }

    /// Get git sources for update command
    pub fn git_sources(&self) -> Vec<GitSource> {
        self.sources
            .iter()
            .filter_map(|s| match s {
                SourceConfig::Git { url, .. } => GitSource::new(url.clone()).ok(),
                _ => None,
            })
            .collect()
    }

    /// Get raw source configs
    pub fn source_configs(&self) -> &[SourceConfig] {
        &self.sources
    }

    /// Add a source to the config
    pub fn add_source(&mut self, source: SourceConfig) {
        // Check if source already exists
        let exists = self.sources.iter().any(|s| match (s, &source) {
            (SourceConfig::Local { path: p1, .. }, SourceConfig::Local { path: p2, .. }) => {
                p1 == p2
            }
            (SourceConfig::Git { url: u1, .. }, SourceConfig::Git { url: u2, .. }) => u1 == u2,
            _ => false,
        });

        if !exists {
            self.sources.push(source);
        }
    }

    /// Move a source from one position to another (for priority)
    pub fn move_source(&mut self, from: usize, to: usize) -> Result<()> {
        if from >= self.sources.len() || to >= self.sources.len() {
            anyhow::bail!("Invalid source index");
        }
        let source = self.sources.remove(from);
        self.sources.insert(to, source);
        Ok(())
    }

    /// Remove a source from the config by path/url or name
    pub fn remove_source(&mut self, path_or_url: &str) -> bool {
        let initial_len = self.sources.len();
        // Expand the input path for comparison (handles ~/foo vs /home/user/foo)
        let input_expanded = expand_tilde(path_or_url);

        self.sources.retain(|s| match s {
            SourceConfig::Local { path, name } => {
                // Compare both the raw string and expanded paths, and also by name
                path != path_or_url
                    && expand_tilde(path) != input_expanded
                    && name.as_deref() != Some(path_or_url)
            }
            SourceConfig::Git { url, name } => {
                url != path_or_url && name.as_deref() != Some(path_or_url)
            }
        });
        self.sources.len() < initial_len
    }

    /// Find a bundle by name across all sources
    pub fn find_bundle(
        &self,
        name: &str,
    ) -> Result<Option<(Box<dyn Source>, crate::bundle::Bundle)>> {
        for source in self.sources() {
            // Skip sources that fail to list (they'll be warned about elsewhere)
            let bundles = match source.list_bundles() {
                Ok(b) => b,
                Err(_) => continue,
            };
            if let Some(bundle) = bundles.into_iter().find(|b| b.name == name) {
                return Ok(Some((source, bundle)));
            }
        }
        Ok(None)
    }

    /// Find a bundle by prefix match across all sources.
    /// Legacy fallback: used when no install manifest exists (pre-manifest installs).
    /// Installed skills use `{bundle}-{name}` folder names, so when exact matching
    /// fails, this tries to find a bundle whose name is a prefix of the installed name.
    /// New installs record bundle info in `.skm.toml` manifests instead.
    pub fn find_bundle_by_prefix(
        &self,
        installed_name: &str,
    ) -> Result<Option<crate::bundle::Bundle>> {
        let mut best_match: Option<crate::bundle::Bundle> = None;
        let mut best_len = 0;

        for source in self.sources() {
            let bundles = match source.list_bundles() {
                Ok(b) => b,
                Err(_) => continue,
            };
            for bundle in bundles {
                let prefix = format!("{}-", bundle.name);
                if installed_name.starts_with(&prefix) && bundle.name.len() > best_len {
                    best_len = bundle.name.len();
                    best_match = Some(bundle);
                }
            }
        }

        Ok(best_match)
    }

    /// Find a source by its name
    pub fn find_source_by_name(&self, name: &str) -> Option<(Box<dyn Source>, &SourceConfig)> {
        for source_config in &self.sources {
            if source_config.name() == Some(name) {
                let source: Option<Box<dyn Source>> = match source_config {
                    SourceConfig::Local { path, .. } => {
                        let expanded = expand_tilde(path);
                        Some(Box::new(LocalSource::new(expanded)))
                    }
                    SourceConfig::Git { url, .. } => GitSource::new(url.clone())
                        .ok()
                        .map(|s| Box::new(s) as Box<dyn Source>),
                };
                if let Some(source) = source {
                    return Some((source, source_config));
                }
            }
        }
        None
    }
}

impl SourceConfig {
    /// Get display string for this source
    pub fn display(&self) -> &str {
        match self {
            SourceConfig::Local { path, .. } => path,
            SourceConfig::Git { url, .. } => url,
        }
    }

    /// Get the optional name for this source
    pub fn name(&self) -> Option<&str> {
        match self {
            SourceConfig::Local { name, .. } => name.as_deref(),
            SourceConfig::Git { name, .. } => name.as_deref(),
        }
    }
}

/// Expand ~ to home directory
fn expand_tilde(path: &str) -> PathBuf {
    if path.starts_with("~/") {
        if let Some(home) = dirs_home() {
            return home.join(&path[2..]);
        }
    } else if path == "~" {
        if let Some(home) = dirs_home() {
            return home;
        }
    }
    PathBuf::from(path)
}

fn dirs_home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expand_tilde() {
        let home = std::env::var("HOME").unwrap();
        assert_eq!(
            expand_tilde("~/.claude-skills"),
            PathBuf::from(format!("{}/.claude-skills", home))
        );
        assert_eq!(
            expand_tilde("/absolute/path"),
            PathBuf::from("/absolute/path")
        );
    }

    #[test]
    fn test_default_config() {
        let config = Config::load_or_default().unwrap();
        assert_eq!(config.default_tool, "claude");
        assert!(!config.sources.is_empty());
    }
}
