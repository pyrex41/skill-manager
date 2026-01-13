# Contributing to Gauntlet Champion Resources

Thanks for contributing! Here's how to add your skills, commands, agents, or rules.

## Quick Start

1. Fork this repo
2. Create your resource folder
3. Add `meta.yaml` + content file
4. Submit a PR

## Directory Structure

```
resources/
├── skills/
│   └── your-skill-name/
│       ├── meta.yaml        # Required: metadata
│       └── skill.md         # Required: content
├── commands/
│   └── your-command-name/
│       ├── meta.yaml
│       └── command.md
├── agents/
│   └── your-agent-name/
│       ├── meta.yaml
│       └── agent.md
└── cursor-rules/
    └── your-rule-name/
        ├── meta.yaml
        └── rule.md
```

## Naming Rules

- **Use kebab-case**: `my-awesome-skill` ✓
- **Lowercase only**: `MySkill` ✗ → `my-skill` ✓
- **No spaces**: `my skill` ✗ → `my-skill` ✓
- **First-write-wins**: If a name is taken, choose a different one

## meta.yaml Format

```yaml
# Required fields
name: My Awesome Skill
author: your-github-username
description: A clear description of what this does.

# Optional fields
tags:
  - productivity
  - debugging
  - testing
version: "1.0.0"
instructions: |
  Additional usage instructions.
  Can span multiple lines.
```

### Required Fields

| Field | Description |
|-------|-------------|
| `name` | Display name (can include spaces) |
| `author` | Your GitHub username |
| `description` | What does this do? (1-2 sentences) |

### Optional Fields

| Field | Description |
|-------|-------------|
| `tags` | Searchable keywords |
| `version` | Semantic version (e.g., "1.0.0") |
| `instructions` | Detailed usage notes |

## Content File

The `.md` file contains the actual skill/command/agent/rule content:

```markdown
# My Awesome Skill

Instructions and context for the AI assistant.

## When to Use

- Use this when...
- This helps with...

## Instructions

1. First, do this...
2. Then, do that...
```

## Validation

Before submitting, run the validator locally:

```bash
./validate-resources.sh
```

Or the CI will check automatically when you open a PR.

## Example: Adding a Skill

1. Create folder: `resources/skills/code-review-helper/`

2. Create `meta.yaml`:
   ```yaml
   name: Code Review Helper
   author: johndoe
   description: Helps perform thorough code reviews with a checklist approach.
   tags:
     - code-review
     - quality
   version: "1.0.0"
   ```

3. Create `skill.md`:
   ```markdown
   # Code Review Helper

   When reviewing code, follow this checklist...
   ```

4. Run validator: `./validate-resources.sh`

5. Submit PR!

## What Gets Checked

The CI validates:

- ✅ Correct directory structure
- ✅ `meta.yaml` exists with required fields
- ✅ Content `.md` file exists
- ✅ No naming conflicts
- ✅ kebab-case naming convention
- ✅ Valid YAML syntax

## Questions?

Open an issue or ask in the Gauntlet community!
