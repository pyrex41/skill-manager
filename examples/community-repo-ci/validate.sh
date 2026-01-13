#!/bin/bash
#
# Validate skill repository structure
# Supports: Resources, Anthropic, and Flat formats
#
# Usage: ./validate.sh [directory]
#

set -e

DIR="${1:-.}"
cd "$DIR"

ERRORS=0
WARNINGS=0
FORMAT=""

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

error() { echo -e "${RED}ERROR:${NC} $1"; ((ERRORS++)); }
warn() { echo -e "${YELLOW}WARNING:${NC} $1"; ((WARNINGS++)); }
info() { echo -e "${BLUE}INFO:${NC} $1"; }
ok() { echo -e "${GREEN}OK:${NC} $1"; }

echo ""
echo "========================================"
echo "  Skill Repository Validator"
echo "========================================"
echo ""

# Detect format
if [ -d "resources" ]; then
  FORMAT="resources"
  info "Detected format: Resources (resources/{type}/name/)"
elif [ -d "skills" ] && find skills -name "SKILL.md" -type f 2>/dev/null | head -1 | grep -q .; then
  FORMAT="anthropic"
  info "Detected format: Anthropic (skills/{name}/SKILL.md)"
elif [ -d "skills" ] || [ -d "commands" ] || [ -d "agents" ] || [ -d "rules" ]; then
  FORMAT="flat"
  info "Detected format: Flat ({skills,commands,agents,rules}/*.md)"
else
  error "No recognized skill format found"
  echo ""
  echo "Expected one of:"
  echo "  - resources/{skills,commands,agents,rules}/name/{meta.yaml,*.md}"
  echo "  - skills/{name}/SKILL.md"
  echo "  - {skills,commands,agents,rules}/*.md"
  exit 1
fi

echo ""

# Validate based on format
case "$FORMAT" in
  resources)
    VALID_TYPES="skills commands agents cursor-rules rules"
    
    for type_dir in resources/*/; do
      [ -d "$type_dir" ] || continue
      type_name=$(basename "$type_dir")
      
      if ! echo "$VALID_TYPES" | grep -qw "$type_name"; then
        error "Invalid resource type: $type_name"
        info "Valid types: $VALID_TYPES"
        continue
      fi
      
      echo "--- $type_name/ ---"
      
      for resource_dir in "$type_dir"*/; do
        [ -d "$resource_dir" ] || continue
        resource_name=$(basename "$resource_dir")
        
        # Skip templates
        [[ "$resource_name" == _* ]] && { info "Skipping template: $resource_name"; continue; }
        [[ "$resource_name" == .* ]] && continue
        
        echo ""
        echo "  $resource_name:"
        
        # Require meta.yaml
        if [ ! -f "$resource_dir/meta.yaml" ]; then
          error "  Missing meta.yaml"
        else
          if ! grep -q "^name:" "$resource_dir/meta.yaml"; then
            error "  meta.yaml missing 'name' field"
          else
            ok "  name field present"
          fi
          if ! grep -q "^author:" "$resource_dir/meta.yaml"; then
            error "  meta.yaml missing 'author' field"
          else
            ok "  author field present"
          fi
          if ! grep -q "^description:" "$resource_dir/meta.yaml"; then
            warn "  meta.yaml missing 'description' field (recommended)"
          fi
        fi
        
        # Require at least one .md file
        md_count=$(find "$resource_dir" -maxdepth 1 -name "*.md" -type f 2>/dev/null | wc -l | tr -d ' ')
        if [ "$md_count" -eq 0 ]; then
          error "  No .md content file found"
        elif [ "$md_count" -gt 1 ]; then
          warn "  Multiple .md files found"
        else
          ok "  Content file present"
        fi
        
        # Warn on naming convention
        if [[ ! "$resource_name" =~ ^[a-z0-9]+(-[a-z0-9]+)*$ ]]; then
          warn "  Name should be kebab-case (e.g., my-skill-name)"
        fi
      done
      echo ""
    done
    ;;
    
  anthropic)
    for skill_dir in skills/*/; do
      [ -d "$skill_dir" ] || continue
      skill_name=$(basename "$skill_dir")
      
      # Skip templates
      [[ "$skill_name" == _* ]] && { info "Skipping template: $skill_name"; continue; }
      [[ "$skill_name" == .* ]] && continue
      
      echo "  $skill_name:"
      
      # Require SKILL.md
      if [ ! -f "$skill_dir/SKILL.md" ]; then
        error "  Missing SKILL.md"
      else
        ok "  SKILL.md present"
        
        # Check for frontmatter
        if ! head -1 "$skill_dir/SKILL.md" | grep -q "^---"; then
          warn "  SKILL.md missing YAML frontmatter (recommended)"
        elif ! grep -q "^name:" "$skill_dir/SKILL.md"; then
          warn "  SKILL.md frontmatter missing 'name' field (recommended)"
        else
          ok "  Frontmatter with name field"
        fi
      fi
      
      # Warn on naming convention
      if [[ ! "$skill_name" =~ ^[a-z0-9]+(-[a-z0-9]+)*$ ]]; then
        warn "  Name should be kebab-case"
      fi
      echo ""
    done
    ;;
    
  flat)
    for type_dir in skills commands agents rules; do
      [ -d "$type_dir" ] || continue
      
      echo "--- $type_dir/ ---"
      
      for md_file in "$type_dir"/*.md; do
        [ -f "$md_file" ] || continue
        filename=$(basename "$md_file")
        
        echo "  $filename:"
        
        # Check file is not empty
        if [ ! -s "$md_file" ]; then
          error "  File is empty"
        else
          ok "  Has content"
        fi
        
        # Warn on naming convention
        name="${filename%.md}"
        if [[ ! "$name" =~ ^[a-z0-9]+(-[a-z0-9]+)*$ ]]; then
          warn "  Name should be kebab-case"
        fi
      done
      echo ""
    done
    ;;
esac

# Check for naming conflicts
echo "--- Naming Conflicts ---"
NAMES_FILE=$(mktemp)

if [ -d "resources" ]; then
  for type_dir in resources/*/; do
    [ -d "$type_dir" ] || continue
    find "$type_dir" -mindepth 1 -maxdepth 1 -type d \
      ! -name "_*" ! -name ".*" -exec basename {} \; >> "$NAMES_FILE"
  done
fi

if [ -d "skills" ]; then
  find skills -mindepth 1 -maxdepth 1 -type d \
    ! -name "_*" ! -name ".*" -exec basename {} \; >> "$NAMES_FILE" 2>/dev/null || true
fi

DUPES=$(cat "$NAMES_FILE" | tr '[:upper:]' '[:lower:]' | sort | uniq -d)
rm "$NAMES_FILE"

if [ -n "$DUPES" ]; then
  error "Naming conflicts found:"
  echo "$DUPES" | while read -r dupe; do
    echo "  - $dupe"
  done
else
  ok "No naming conflicts"
fi

echo ""
echo "========================================"
echo "  Summary"
echo "========================================"
echo ""
echo "  Format:   $FORMAT"
echo "  Errors:   $ERRORS"
echo "  Warnings: $WARNINGS"
echo ""

if [ "$ERRORS" -gt 0 ]; then
  echo -e "${RED}VALIDATION FAILED${NC}"
  echo ""
  echo "Please fix the errors above before submitting your PR."
  exit 1
elif [ "$WARNINGS" -gt 0 ]; then
  echo -e "${YELLOW}VALIDATION PASSED WITH WARNINGS${NC}"
  echo ""
  echo "Consider addressing the warnings above."
  exit 0
else
  echo -e "${GREEN}VALIDATION PASSED${NC}"
  exit 0
fi
