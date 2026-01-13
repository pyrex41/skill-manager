use anyhow::{Context, Result};
use colored::Colorize;
use std::path::PathBuf;

use crate::bundle::Bundle;

/// Trait for skill sources (local directories, git repos, etc.)
pub trait Source {
    /// List all bundles in this source
    fn list_bundles(&self) -> Result<Vec<Bundle>>;

    /// Get display path for this source
    fn display_path(&self) -> String;
}

/// A local directory source
pub struct LocalSource {
    path: PathBuf,
}

impl LocalSource {
    pub fn new(path: PathBuf) -> Self {
        LocalSource { path }
    }
}

impl Source for LocalSource {
    fn list_bundles(&self) -> Result<Vec<Bundle>> {
        if !self.path.exists() {
            return Ok(vec![]);
        }

        // Check if this is a resources-format source (has resources/ directory at root)
        // Each resource folder becomes its own bundle
        if Bundle::is_resources_format(&self.path) {
            return Bundle::list_from_resources_path(self.path.clone());
        }

        let mut bundles = vec![];

        for entry in std::fs::read_dir(&self.path)? {
            let entry = entry?;
            let path = entry.path();

            // Skip non-directories
            if !path.is_dir() {
                continue;
            }

            // Skip hidden directories and 'shell' directory
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

            if name.starts_with('.') || name == "shell" {
                continue;
            }

            // Try to create a bundle from this directory
            match Bundle::from_path(path) {
                Ok(bundle) if !bundle.is_empty() => bundles.push(bundle),
                _ => continue,
            }
        }

        // Sort bundles by name
        bundles.sort_by(|a, b| a.name.cmp(&b.name));

        Ok(bundles)
    }

    fn display_path(&self) -> String {
        // Try to show with ~ if it's under home
        if let Some(home) = std::env::var_os("HOME") {
            let home_path = PathBuf::from(home);
            if let Ok(relative) = self.path.strip_prefix(&home_path) {
                return format!("~/{}", relative.display());
            }
        }
        self.path.display().to_string()
    }
}

/// A git repository source
pub struct GitSource {
    url: String,
    cache_path: PathBuf,
}

impl GitSource {
    pub fn new(url: String) -> Result<Self> {
        let cache_path = Self::cache_path_for_url(&url)?;
        Ok(GitSource { url, cache_path })
    }

    /// Get the cache directory for a git URL
    fn cache_path_for_url(url: &str) -> Result<PathBuf> {
        let cache_dir = directories::ProjectDirs::from("", "", "skm")
            .ok_or_else(|| anyhow::anyhow!("Could not determine cache directory"))?
            .cache_dir()
            .to_path_buf();

        // Parse URL to create a path like github.com/user/repo
        let path_suffix = Self::url_to_path(url);
        Ok(cache_dir.join(path_suffix))
    }

    /// Convert a git URL to a filesystem path
    fn url_to_path(url: &str) -> String {
        // Handle various URL formats:
        // https://github.com/user/repo.git -> github.com/user/repo
        // git@github.com:user/repo.git -> github.com/user/repo
        // https://github.com/user/repo -> github.com/user/repo

        let url = url.trim_end_matches(".git");

        if url.starts_with("https://") {
            url.strip_prefix("https://").unwrap_or(url).to_string()
        } else if url.starts_with("git@") {
            // git@github.com:user/repo -> github.com/user/repo
            url.strip_prefix("git@").unwrap_or(url).replace(':', "/")
        } else {
            url.to_string()
        }
    }

    /// Clone the repository if it doesn't exist
    pub fn ensure_cloned(&self) -> Result<()> {
        if self.cache_path.exists() {
            return Ok(());
        }

        println!("  {} {}...", "Cloning".cyan(), self.url);

        // Create parent directory
        if let Some(parent) = self.cache_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Clone the repository
        git2::Repository::clone(&self.url, &self.cache_path)
            .with_context(|| format!("Failed to clone {}", self.url))?;

        Ok(())
    }

    /// Get the URL for display
    pub fn url(&self) -> &str {
        &self.url
    }

    /// Pull latest changes from the remote
    pub fn pull(&self) -> Result<bool> {
        if !self.cache_path.exists() {
            self.ensure_cloned()?;
            return Ok(true);
        }

        let repo = git2::Repository::open(&self.cache_path)
            .with_context(|| format!("Failed to open repository at {:?}", self.cache_path))?;

        // Fetch from origin
        let mut remote = repo.find_remote("origin")?;
        remote.fetch(&["HEAD"], None, None)?;

        // Get the fetch head
        let fetch_head = repo.find_reference("FETCH_HEAD")?;
        let fetch_commit = repo.reference_to_annotated_commit(&fetch_head)?;

        // Get HEAD
        let head = repo.head()?;
        let head_commit = head.peel_to_commit()?;

        // Check if we need to update
        if fetch_commit.id() == head_commit.id() {
            return Ok(false);
        }

        // Fast-forward merge
        let refname = head.name().unwrap_or("HEAD");
        repo.reference(refname, fetch_commit.id(), true, "Fast-forward")?;
        repo.set_head(refname)?;
        repo.checkout_head(Some(git2::build::CheckoutBuilder::default().force()))?;

        Ok(true)
    }
}

impl Source for GitSource {
    fn list_bundles(&self) -> Result<Vec<Bundle>> {
        // Ensure the repo is cloned first
        self.ensure_cloned()?;

        // Delegate to LocalSource for actual bundle discovery
        let local = LocalSource::new(self.cache_path.clone());
        local.list_bundles()
    }

    fn display_path(&self) -> String {
        self.url.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_local_source_empty_dir() {
        let dir = tempdir().unwrap();
        let source = LocalSource::new(dir.path().to_path_buf());
        let bundles = source.list_bundles().unwrap();
        assert!(bundles.is_empty());
    }

    #[test]
    fn test_local_source_with_bundle() {
        let dir = tempdir().unwrap();

        // Create a bundle with a command
        let bundle_dir = dir.path().join("test-bundle");
        let commands_dir = bundle_dir.join("commands");
        fs::create_dir_all(&commands_dir).unwrap();
        fs::write(commands_dir.join("test.md"), "# Test command").unwrap();

        let source = LocalSource::new(dir.path().to_path_buf());
        let bundles = source.list_bundles().unwrap();

        assert_eq!(bundles.len(), 1);
        assert_eq!(bundles[0].name, "test-bundle");
        assert_eq!(bundles[0].commands.len(), 1);
        assert_eq!(bundles[0].commands[0].name, "test");
    }

    #[test]
    fn test_local_source_skips_hidden_and_shell() {
        let dir = tempdir().unwrap();

        // Create hidden directory
        let hidden = dir.path().join(".hidden");
        fs::create_dir_all(hidden.join("commands")).unwrap();
        fs::write(hidden.join("commands/test.md"), "# Test").unwrap();

        // Create shell directory
        let shell = dir.path().join("shell");
        fs::create_dir_all(&shell).unwrap();
        fs::write(shell.join("skim.bash"), "# Shell script").unwrap();

        let source = LocalSource::new(dir.path().to_path_buf());
        let bundles = source.list_bundles().unwrap();

        assert!(bundles.is_empty());
    }

    #[test]
    fn test_local_source_resources_format() {
        let dir = tempdir().unwrap();

        // Create resources-format structure with multiple resources
        let resources = dir.path().join("resources");
        let skills_dir = resources.join("skills");

        // First skill
        let skill1 = skills_dir.join("my-skill");
        fs::create_dir_all(&skill1).unwrap();
        fs::write(skill1.join("meta.yaml"), "name: My Skill\nauthor: test\n").unwrap();
        fs::write(skill1.join("skill.md"), "# Skill content").unwrap();

        // Second skill
        let skill2 = skills_dir.join("another-skill");
        fs::create_dir_all(&skill2).unwrap();
        fs::write(
            skill2.join("meta.yaml"),
            "name: Another Skill\nauthor: test\n",
        )
        .unwrap();
        fs::write(skill2.join("skill.md"), "# Another skill").unwrap();

        let source = LocalSource::new(dir.path().to_path_buf());
        let bundles = source.list_bundles().unwrap();

        // Each resource folder becomes its own bundle
        assert_eq!(bundles.len(), 2);
        assert_eq!(bundles[0].name, "Another Skill");
        assert_eq!(bundles[1].name, "My Skill");
    }
}
