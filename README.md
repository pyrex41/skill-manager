# skill-manager (skm)

A CLI tool for managing AI coding assistant skills across Claude, OpenCode, and Cursor.

## What This Does

AI coding assistants like Claude Code, OpenCode, and Cursor support custom "skills" - markdown files containing prompts, instructions, or agent definitions that extend their capabilities. The problem: each tool expects these files in different locations with different formats.

**skill-manager** lets you maintain a single collection of skills and install them to any supported tool. It handles the path conventions and file transformations automatically.

## How It Works

```
┌─────────────────┐      ┌─────────────────┐      ┌─────────────────┐
│     Sources     │      │     Bundles     │      │     Targets     │
│                 │      │                 │      │                 │
│ ~/.claude-skills│ ───► │ my-bundle/      │ ───► │ .claude/        │
│ ~/my-skills     │      │   skills/       │      │ .opencode/      │
│ github.com/...  │      │   agents/       │      │ .cursor/        │
└─────────────────┘      │   commands/     │      └─────────────────┘
                         └─────────────────┘
```

1. **Sources** are directories (local or git repos) containing skill bundles
2. **Bundles** are folders with `skills/`, `agents/`, and/or `commands/` subdirectories
3. **Targets** are the tool-specific directories where skills get installed

When you run `skm my-bundle`, it copies the bundle's files to the appropriate locations for your chosen tool, applying any necessary transformations.

## Installation

```bash
cargo install skill-manager
```

This installs the `skm` binary. Requires [Rust](https://rustup.rs/) to be installed.

## Quick Start

```bash
# Browse available bundles interactively
skm list

# Install a bundle to Claude (default)
skm add my-bundle
# or just:
skm my-bundle

# Install to OpenCode or Cursor instead
skm my-bundle -o    # OpenCode
skm my-bundle -c    # Cursor

# Manage sources interactively
skm sources

# See what's installed in current directory
skm here

# Remove installed skills interactively
skm here --remove
```

## Commands

### `skm list`
Interactive browser for exploring available bundles. Navigate through sources, view bundle contents, and inspect individual skill files.

### `skm add <bundle>` or `skm <bundle>`
Install a bundle to the current directory. Bundles are searched in priority order across all configured sources.

```bash
skm add my-bundle         # Install to Claude (default)
skm add my-bundle -o      # Install to OpenCode
skm add my-bundle -c      # Install to Cursor
skm add my-bundle -g      # Install globally
skm add my-bundle --skills    # Install only skills
skm add my-bundle --agents    # Install only agents
skm add my-bundle --commands  # Install only commands
```

### `skm sources`
Interactive menu to view, add, remove, and reorder sources by priority. Sources are checked in order when searching for bundles.

```bash
skm sources           # Interactive management
skm sources list      # Just list sources
skm sources add <path>    # Add a local directory or git URL
skm sources remove <path> # Remove a source
```

### `skm here`
Show and manage skills installed in the current directory.

```bash
skm here                # Show all installed skills
skm here --tool claude  # Filter by tool
skm here --remove       # Interactive removal
skm here --clean        # Remove all (with confirmation)
skm here --clean --yes  # Remove all without confirmation
```

### `skm update`
Pull latest changes from all git sources.

## Supported Skill Formats

skm supports multiple skill repository formats, making it compatible with popular community skill repos.

### Flat Bundle Format

The original format - a directory with subdirectories for each type:

```
my-bundle/
├── skills/          # Reusable skill definitions
│   └── helper.md
├── agents/          # Agent definitions
│   └── reviewer.md
├── commands/        # Slash commands (e.g., /commit)
│   └── commit.md
└── rules/           # Rules/guidelines
    └── style.md
```

### Anthropic/Marketplace Format

Compatible with [anthropics/skills](https://github.com/anthropics/skills) and [huggingface/skills](https://github.com/huggingface/skills):

```
skills/
├── xlsx/
│   └── SKILL.md     # With YAML frontmatter (name, description)
├── pdf/
│   └── SKILL.md
└── docx/
    └── SKILL.md
```

Each skill folder becomes a separate installable bundle. The skill name is extracted from YAML frontmatter if present.

```bash
# Add the official Anthropic skills repo
skm sources add https://github.com/anthropics/skills

# Install individual skills
skm xlsx
skm pdf
```

### Community Resources Format

For community repos with `resources/` directory structure:

```
resources/
├── skills/
│   └── my-skill/
│       ├── meta.yaml    # name, author, description
│       └── skill.md
└── commands/
    └── my-command/
        ├── meta.yaml
        └── command.md
```

Each resource folder becomes a separate bundle, named from `meta.yaml`.

### Where Files Get Installed

| Source | Claude | OpenCode | Cursor |
|--------|--------|----------|--------|
| `skills/foo.md` | `.claude/skills/bundle/foo.md` | `.opencode/skill/bundle-foo/SKILL.md` | `.cursor/skills/bundle-foo/SKILL.md` |
| `agents/foo.md` | `.claude/agents/bundle/foo.md` | `.opencode/agent/bundle-foo.md` | `.cursor/rules/bundle-foo/RULE.md` |
| `commands/foo.md` | `.claude/commands/bundle/foo.md` | `.opencode/command/bundle-foo.md` | `.cursor/rules/bundle-foo/RULE.md` |
| `rules/foo.md` | `.claude/rules/bundle/foo.md` | `.opencode/rule/bundle-foo/RULE.md` | `.cursor/rules/bundle-foo/RULE.md` |

OpenCode and Cursor skills/rules require YAML frontmatter with a `name` field - skm adds this automatically if missing.

## Configuration

Config file: `~/.config/skm/config.toml`

```toml
default_tool = "claude"

[[sources]]
type = "local"
path = "~/.claude-skills"

[[sources]]
type = "git"
url = "https://github.com/user/skills"
```

Sources are searched in order (first match wins). Use `skm sources` to manage priority.

## Shell Completions

```bash
skm completions bash > ~/.local/share/bash-completion/completions/skm
skm completions zsh > ~/.zfunc/_skm
skm completions fish > ~/.config/fish/completions/skm.fish
```

## License

MIT
