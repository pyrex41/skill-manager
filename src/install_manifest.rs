use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::target::Tool;

/// Tracks which bundles are installed in a target directory per tool.
/// Stored as `.claude/.skm.toml`, `.opencode/.skm.toml`, etc.
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct InstallManifest {
    #[serde(default)]
    pub bundles: Vec<ManifestEntry>,
}

/// A single installed bundle record.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct ManifestEntry {
    pub name: String,
    pub source: String,
}

impl InstallManifest {
    /// Returns the manifest file path for a given tool and target directory.
    /// e.g. `target/.claude/.skm.toml`
    pub fn path_for(tool: &Tool, target_dir: &Path) -> PathBuf {
        target_dir.join(tool.tool_dir_name()).join(".skm.toml")
    }

    /// Load the manifest for a tool. Returns empty manifest if missing or corrupt.
    pub fn load(tool: &Tool, target_dir: &Path) -> Self {
        let path = Self::path_for(tool, target_dir);
        match std::fs::read_to_string(&path) {
            Ok(content) => match toml::from_str(&content) {
                Ok(manifest) => manifest,
                Err(e) => {
                    eprintln!(
                        "Warning: corrupt install manifest at {}: {}",
                        path.display(),
                        e
                    );
                    Self::default()
                }
            },
            Err(_) => Self::default(),
        }
    }

    /// Save the manifest for a tool.
    pub fn save(&self, tool: &Tool, target_dir: &Path) -> anyhow::Result<()> {
        let path = Self::path_for(tool, target_dir);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(self)?;
        std::fs::write(&path, content)?;
        Ok(())
    }

    /// Record a bundle install (upsert: update source if exists, append if new).
    pub fn record_install(&mut self, name: &str, source: &str) {
        if let Some(entry) = self.bundles.iter_mut().find(|e| e.name == name) {
            entry.source = source.to_string();
        } else {
            self.bundles.push(ManifestEntry {
                name: name.to_string(),
                source: source.to_string(),
            });
        }
    }

    /// Remove a bundle entry by name. Returns true if an entry was removed.
    pub fn remove_bundle(&mut self, name: &str) -> bool {
        let len_before = self.bundles.len();
        self.bundles.retain(|e| e.name != name);
        self.bundles.len() < len_before
    }

    /// Get all recorded bundle names.
    pub fn bundle_names(&self) -> Vec<&str> {
        self.bundles.iter().map(|e| e.name.as_str()).collect()
    }

    /// Check if the manifest has any entries.
    pub fn is_empty(&self) -> bool {
        self.bundles.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_path_for() {
        let dir = PathBuf::from("/tmp/project");
        assert_eq!(
            InstallManifest::path_for(&Tool::Claude, &dir),
            PathBuf::from("/tmp/project/.claude/.skm.toml")
        );
        assert_eq!(
            InstallManifest::path_for(&Tool::OpenCode, &dir),
            PathBuf::from("/tmp/project/.opencode/.skm.toml")
        );
        assert_eq!(
            InstallManifest::path_for(&Tool::Cursor, &dir),
            PathBuf::from("/tmp/project/.cursor/.skm.toml")
        );
        assert_eq!(
            InstallManifest::path_for(&Tool::Codex, &dir),
            PathBuf::from("/tmp/project/.codex/.skm.toml")
        );
    }

    #[test]
    fn test_roundtrip_save_load() {
        let dir = tempdir().unwrap();
        let target = dir.path();

        let mut manifest = InstallManifest::default();
        manifest.record_install("ralph", "~/claude_skills");
        manifest.record_install("cl", "https://github.com/example/repo");

        manifest.save(&Tool::Claude, target).unwrap();

        let loaded = InstallManifest::load(&Tool::Claude, target);
        assert_eq!(loaded.bundles.len(), 2);
        assert_eq!(loaded.bundles[0].name, "ralph");
        assert_eq!(loaded.bundles[0].source, "~/claude_skills");
        assert_eq!(loaded.bundles[1].name, "cl");
        assert_eq!(
            loaded.bundles[1].source,
            "https://github.com/example/repo"
        );
    }

    #[test]
    fn test_upsert_idempotency() {
        let mut manifest = InstallManifest::default();
        manifest.record_install("ralph", "~/old_path");
        manifest.record_install("ralph", "~/new_path");

        assert_eq!(manifest.bundles.len(), 1);
        assert_eq!(manifest.bundles[0].source, "~/new_path");
    }

    #[test]
    fn test_remove_bundle() {
        let mut manifest = InstallManifest::default();
        manifest.record_install("ralph", "~/skills");
        manifest.record_install("cl", "https://example.com");

        assert!(manifest.remove_bundle("ralph"));
        assert_eq!(manifest.bundles.len(), 1);
        assert_eq!(manifest.bundles[0].name, "cl");

        // Removing non-existent returns false
        assert!(!manifest.remove_bundle("nonexistent"));
    }

    #[test]
    fn test_bundle_names() {
        let mut manifest = InstallManifest::default();
        manifest.record_install("ralph", "~/skills");
        manifest.record_install("cl", "https://example.com");

        let names = manifest.bundle_names();
        assert_eq!(names, vec!["ralph", "cl"]);
    }

    #[test]
    fn test_load_missing_file() {
        let dir = tempdir().unwrap();
        let manifest = InstallManifest::load(&Tool::Claude, dir.path());
        assert!(manifest.bundles.is_empty());
    }

    #[test]
    fn test_load_corrupt_file() {
        let dir = tempdir().unwrap();
        let tool_dir = dir.path().join(".claude");
        std::fs::create_dir_all(&tool_dir).unwrap();
        std::fs::write(tool_dir.join(".skm.toml"), "not valid toml {{{{").unwrap();

        let manifest = InstallManifest::load(&Tool::Claude, dir.path());
        assert!(manifest.bundles.is_empty());
    }

    #[test]
    fn test_is_empty() {
        let manifest = InstallManifest::default();
        assert!(manifest.is_empty());

        let mut manifest = InstallManifest::default();
        manifest.record_install("test", "source");
        assert!(!manifest.is_empty());
    }
}
