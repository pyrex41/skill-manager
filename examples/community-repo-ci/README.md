# Community Skill Repository CI

This directory contains GitHub Actions and scripts for validating skill contributions in community repositories.

## Features

- **Multi-format support**: Validates Resources, Anthropic, and Flat skill formats
- **Bypass mechanism**: Add `skip-validation` label to skip checks on maintenance PRs
- **Local validation**: Run `validate.sh` before submitting PRs
- **Naming conflict detection**: Prevents duplicate skill names

## Setup

1. Copy `.github/workflows/validate-skills.yml` to your repository's `.github/workflows/` directory

2. (Optional) Copy `validate.sh` to your repository root for local validation

3. (Optional) Create the `skip-validation` label in your GitHub repository settings

## Supported Formats

### Resources Format
```
resources/
├── skills/
│   └── my-skill/
│       ├── meta.yaml      # Required: name, author; Recommended: description
│       └── skill.md       # Content file
├── commands/
│   └── my-command/
│       ├── meta.yaml
│       └── command.md
└── cursor-rules/
    └── my-rule/
        ├── meta.yaml
        └── rule.md
```

### Anthropic Format
```
skills/
├── xlsx/
│   └── SKILL.md           # With YAML frontmatter (name, description)
├── pdf/
│   └── SKILL.md
└── docx/
    └── SKILL.md
```

### Flat Format
```
skills/
├── helper.md
└── analyzer.md
commands/
├── commit.md
└── review.md
```

## Validation Rules

### All Formats
- At least one skill/command/agent must exist
- Names should be kebab-case (lowercase with hyphens)
- No duplicate names (case-insensitive)

### Resources Format
- Each resource folder must have `meta.yaml`
- `meta.yaml` must include `name` and `author` fields
- `description` field is recommended
- At least one `.md` content file required

### Anthropic Format
- Each skill folder must have `SKILL.md`
- YAML frontmatter with `name` field is recommended

### Flat Format
- Files must not be empty

## Bypassing Validation

For maintenance PRs that don't add skills (e.g., updating README, CI fixes):

1. Add the `skip-validation` label to the PR
2. The validation workflow will be skipped

## Local Validation

Run before submitting a PR:

```bash
./validate.sh
```

Or validate a specific directory:

```bash
./validate.sh /path/to/skill-repo
```

## Example CONTRIBUTING.md

See the `gauntlet-ci/CONTRIBUTING.md` file for a template you can adapt for your repository.
