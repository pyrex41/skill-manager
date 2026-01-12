# Plan: Bundle Metadata and Interactive Filtering

## Overview

Add support for `metadata.yaml` files in skill bundles to track author information (GitHub username required, display name optional) and descriptions. Enable interactive filtering by author in the CLI's browse/list mode.

## Current State Analysis

- Bundles are directories containing `skills/`, `agents/`, `commands/` subdirectories with `.md` files
- `Bundle` struct (`bundle.rs:34-46`) tracks: name, path, skills, agents, commands
- No metadata is currently stored or displayed
- `skm list` shows bundles with counts but no author/description info
- Dependencies include `serde` but not `serde_yaml`

## Desired End State

1. Each bundle can have an optional `metadata.yaml` file at its root:
   ```yaml
   github: "pyrex41"              # required
   author: "Reuben Brooks"        # optional display name
   description: "Git workflow commands for Claude Code"  # optional
   ```

2. `skm list` displays author information:
   ```
   Available bundles:
     cl          3s 2a 1c  by Reuben Brooks (@pyrex41)
     gastro      5s 0a 3c  by @someuser
   ```

3. Interactive filtering when running `skm list`:
   ```
   Filter by author? [All] [pyrex41] [someuser] [Type to search...]
   > pyrex41

   Bundles by @pyrex41:
     cl          3s 2a 1c  Git workflow commands
   ```

## Implementation Approach

Add metadata support in three incremental phases:
1. Parse and store metadata in Bundle struct
2. Display metadata in list/browse output
3. Add interactive filtering

## Phases

### Phase 1: Metadata Parsing

**Goal**: Parse `metadata.yaml` files and store in Bundle struct

**Changes**:

- [ ] Add `serde_yaml` dependency (`Cargo.toml:24`)
  ```toml
  serde_yaml = "0.9"
  ```

- [ ] Create metadata struct (`bundle.rs:31` - insert before Bundle struct)
  ```rust
  /// Metadata for a skill bundle
  #[derive(Debug, Clone, Default, serde::Deserialize)]
  pub struct BundleMetadata {
      /// GitHub username (required in file, but optional in struct for backwards compat)
      pub github: Option<String>,
      /// Display name (optional)
      pub author: Option<String>,
      /// Bundle description (optional)
      pub description: Option<String>,
  }

  impl BundleMetadata {
      /// Load metadata from a bundle directory, returns default if not found
      pub fn from_path(bundle_path: &std::path::Path) -> Self {
          let meta_path = bundle_path.join("metadata.yaml");
          if meta_path.exists() {
              if let Ok(contents) = std::fs::read_to_string(&meta_path) {
                  if let Ok(meta) = serde_yaml::from_str(&contents) {
                      return meta;
                  }
              }
          }
          Self::default()
      }

      /// Get display name: author if set, otherwise @github, otherwise None
      pub fn display_name(&self) -> Option<String> {
          if let Some(ref author) = self.author {
              if let Some(ref github) = self.github {
                  Some(format!("{} (@{})", author, github))
              } else {
                  Some(author.clone())
              }
          } else {
              self.github.as_ref().map(|g| format!("@{}", g))
          }
      }
  }
  ```

- [ ] Add metadata field to Bundle struct (`bundle.rs:34-46`)
  ```rust
  pub struct Bundle {
      pub name: String,
      pub path: PathBuf,
      pub skills: Vec<SkillFile>,
      pub agents: Vec<SkillFile>,
      pub commands: Vec<SkillFile>,
      pub metadata: BundleMetadata,  // NEW
  }
  ```

- [ ] Load metadata in `Bundle::from_path()` (`bundle.rs:50-68`)
  ```rust
  // After line 59, before Ok(Bundle {...})
  let metadata = BundleMetadata::from_path(&path);

  // Add to struct initialization
  Ok(Bundle {
      name,
      path,
      skills,
      agents,
      commands,
      metadata,  // NEW
  })
  ```

**Success Criteria - Automated**:
- [ ] `cargo build` succeeds
- [ ] `cargo test` passes
- [ ] Add test: bundle with metadata.yaml parses correctly
- [ ] Add test: bundle without metadata.yaml gets default (empty) metadata

**Success Criteria - Manual**:
- [ ] Create test bundle with metadata.yaml, verify it loads

---

### Phase 2: Display Metadata in List

**Goal**: Show author and description in `skm list` output

**Changes**:

- [ ] Update list display in `main.rs` (find the list command handler)
  - Show author after bundle counts: `bundle-name  3s 2a 1c  by @username`
  - Use dim/gray color for author info
  - Truncate description to first 40 chars if shown

- [ ] Update bundle detail view (when selecting a bundle in browse)
  - Show full description
  - Show GitHub profile link: `https://github.com/{github}`

**Success Criteria - Automated**:
- [ ] `cargo build` succeeds
- [ ] `cargo test` passes

**Success Criteria - Manual**:
- [ ] `skm list` shows author info for bundles with metadata
- [ ] `skm list` shows no author for bundles without metadata (graceful)
- [ ] Selecting a bundle shows full description

---

### Phase 3: Interactive Filtering

**Goal**: Add interactive author filter before showing bundle list

**Changes**:

- [ ] Collect all unique authors from bundles
  ```rust
  fn collect_authors(bundles: &[Bundle]) -> Vec<String> {
      let mut authors: Vec<String> = bundles
          .iter()
          .filter_map(|b| b.metadata.github.clone())
          .collect();
      authors.sort();
      authors.dedup();
      authors
  }
  ```

- [ ] Add filter prompt before list display (using `dialoguer::FuzzySelect` or `Select`)
  - Options: "All authors" + list of github usernames
  - Allow typing to fuzzy-filter the list
  - Skip prompt if only one author or `--no-filter` flag

- [ ] Filter bundles by selected author before display
  ```rust
  let filtered: Vec<&Bundle> = if selected_author == "All" {
      bundles.iter().collect()
  } else {
      bundles.iter()
          .filter(|b| b.metadata.github.as_deref() == Some(&selected_author))
          .collect()
  };
  ```

- [ ] Add `--no-filter` or `--all` flag to skip interactive prompt

**Success Criteria - Automated**:
- [ ] `cargo build` succeeds
- [ ] `cargo test` passes

**Success Criteria - Manual**:
- [ ] `skm list` prompts for author filter
- [ ] Selecting an author shows only their bundles
- [ ] "All authors" shows everything
- [ ] `skm list --all` skips the filter prompt
- [ ] Works with 0 authors (no metadata files) - skips filter

---

## Open Questions

None - all decisions confirmed:
- Bundle-level metadata (not repo-level)
- Fields: `github` (required), `author` (optional), `description` (optional)
- Interactive filtering in browse mode
- User-configured sources (no built-in registry)

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| YAML parse errors in malformed metadata.yaml | Silent fallback to default, log warning |
| Performance with many bundles | Metadata is small, parsing is fast. No concern. |
| Breaking change for existing bundles | Fully backwards compatible - metadata is optional |
| Users confused by filter prompt | Add `--all` flag to skip, show clear "All authors" option |

## Future Enhancements (Out of Scope)

- Filter by tags (would need `tags` field in metadata)
- Full-text search across descriptions
- Validation that `github` field is actually a valid GitHub username
- Automatic metadata generation from git commit author
