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
| Skills | `.claude/skills/{bundle}/{name}.md` | `.opencode/skill/{bundle}-{name}/SKILL.md` | `.cursor/skills/{bundle}-{name}/SKILL.md` (beta) |
| Agents | `.claude/agents/{bundle}/{name}.md` | `.opencode/agent/{bundle}-{name}.md` | `.cursor/rules/{bundle}-{name}/RULE.md` |
| Commands | `.claude/commands/{bundle}/{name}.md` | `.opencode/command/{bundle}-{name}.md` | `.cursor/rules/{bundle}-{name}/RULE.md` |
| Rules | `.claude/rules/{bundle}/{name}.md` | `.opencode/rule/{bundle}-{name}/RULE.md` | `.cursor/rules/{bundle}-{name}/RULE.md` |

OpenCode and Cursor skills/rules require YAML frontmatter with a `name` field - `target.rs:transform_skill_file()` adds this automatically.

**Cursor Support**: Cursor supports both folder-based skills (beta, `.cursor/skills/`) and rules (`.cursor/rules/`). Skills are installed to the beta skills directory, while agents, commands, and rules go to the rules directory.

### Key Dependencies

- `clap` - CLI argument parsing with derive macros
- `git2` - Git operations (clone, fetch, fast-forward)
- `dialoguer` - Interactive prompts for setup wizard and skill removal
- `walkdir` - Recursive directory traversal for discovery

## SCUD Task Management

This project uses SCUD Task Manager for task management.

### Session Workflow

1. **Start of session**: Run `scud warmup` to orient yourself
   - Shows current working directory and recent git history
   - Displays active tag, task counts, and any stale locks
   - Identifies the next available task

2. **Claim a task**: Use `/scud:task-next` or `scud next --claim --name "Claude"`
   - Always claim before starting work to prevent conflicts
   - Task context is stored in `.scud/current-task`

3. **Work on the task**: Implement the requirements
   - Reference task details with `/scud:task-show <id>`
   - Dependencies are automatically tracked by the DAG

4. **Commit with context**: Use `scud commit -m "message"` or `scud commit -a -m "message"`
   - Automatically prefixes commits with `[TASK-ID]`
   - Uses task title as default commit message if none provided

5. **Complete the task**: Mark done with `/scud:task-status <id> done`
   - The stop hook will prompt for task completion

### Progress Journaling

Keep a brief progress log during complex tasks:

```
## Progress Log

### Session: 2025-01-15
- Investigated auth module, found issue in token refresh
- Updated refresh logic to handle edge case
- Tests passing, ready for review
```

This helps maintain continuity across sessions and provides context for future work.

### Key Commands

- `scud warmup` - Session orientation
- `scud next` - Find next available task
- `scud show <id>` - View task details
- `scud set-status <id> <status>` - Update task status
- `scud commit` - Task-aware git commit
- `scud stats` - View completion statistics
