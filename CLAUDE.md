# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Test Commands

```bash
cargo build              # Debug build
cargo build --release    # Release build
cargo test               # Run all tests
cargo test <test_name>   # Run a single test
cargo run                # Run the CLI (lists bundles)
cargo run -- <bundle>    # Install a bundle
```

The binary is named `skm` (skill-manager).

## Architecture

This is a Rust CLI tool for managing AI coding assistant skills across Claude, OpenCode, and Cursor. It copies skill bundles from configured sources to tool-specific locations.

### Core Data Flow

1. **Sources** (`source.rs`) - Skill bundles come from local directories or git repositories
2. **Bundles** (`bundle.rs`) - A bundle is a directory containing `skills/`, `agents/`, and/or `commands/` subdirectories with `.md` files
3. **Targets** (`target.rs`) - Each tool (Claude/OpenCode/Cursor) has different destination paths and file transformations
4. **Install** (`install.rs`) - Orchestrates copying from source bundle to target tool location

### Module Responsibilities

- `main.rs` - CLI argument parsing (clap) and command dispatch
- `config.rs` - TOML config file at `~/.config/skm/config.toml`, manages source list
- `source.rs` - `Source` trait with `LocalSource` and `GitSource` implementations; git sources clone to cache dir
- `bundle.rs` - `Bundle` struct representing a skill bundle with its files
- `target.rs` - `Tool` enum handling per-tool path conventions and file transformations
- `discover.rs` - Scans directories for installed skills across all three tools
- `install.rs` - Copies bundle files to target locations
- `setup.rs` - First-run interactive setup wizard

### Tool-Specific Path Mappings

| Type | Claude | OpenCode | Cursor |
|------|--------|----------|--------|
| Skills | `.claude/skills/{bundle}/{name}.md` | `.opencode/skill/{bundle}-{name}/SKILL.md` | `.cursor/skills/{bundle}-{name}/SKILL.md` |
| Agents | `.claude/agents/{bundle}/{name}.md` | `.opencode/agent/{bundle}-{name}.md` | `.cursor/rules/{bundle}-{name}.mdc` |
| Commands | `.claude/commands/{bundle}/{name}.md` | `.opencode/command/{bundle}-{name}.md` | `.cursor/rules/{bundle}-{name}.mdc` |

OpenCode and Cursor skills require YAML frontmatter with a `name` field - `target.rs:transform_skill_file()` adds this automatically.

### Key Dependencies

- `clap` - CLI argument parsing with derive macros
- `git2` - Git operations (clone, fetch, fast-forward)
- `dialoguer` - Interactive prompts for setup wizard and skill removal
- `walkdir` - Recursive directory traversal for discovery
