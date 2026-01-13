#!/bin/bash
#
# Validate Gauntlet Champion Resources structure
# Run this locally before submitting a PR, or use in CI/CD
#
# Usage: ./validate-resources.sh [resources_dir]
#

set -e

RESOURCES_DIR="${1:-resources}"
ERRORS=0
WARNINGS=0

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

error() { echo -e "${RED}❌ ERROR:${NC} $1"; ((ERRORS++)); }
warn() { echo -e "${YELLOW}⚠️  WARNING:${NC} $1"; ((WARNINGS++)); }
info() { echo -e "${BLUE}ℹ️ ${NC} $1"; }
ok() { echo -e "${GREEN}✓${NC} $1"; }

echo ""
echo "═══════════════════════════════════════════════════════"
echo "  Gauntlet Champion Resources Validator"
echo "═══════════════════════════════════════════════════════"
echo ""

if [ ! -d "$RESOURCES_DIR" ]; then
  error "Resources directory not found: $RESOURCES_DIR"
  exit 1
fi

# Valid resource types
VALID_TYPES=("skills" "commands" "agents" "cursor-rules" "rules")

# Track all names for conflict detection
declare -A ALL_NAMES

validate_meta_yaml() {
  local meta_file="$1"
  local resource_path="$2"
  
  if [ ! -f "$meta_file" ]; then
    error "$resource_path: Missing meta.yaml"
    return 1
  fi
  
  local has_error=0
  
  # Check required fields
  if ! grep -q "^name:" "$meta_file"; then
    error "$resource_path: meta.yaml missing required 'name' field"
    has_error=1
  fi
  
  if ! grep -q "^author:" "$meta_file"; then
    error "$resource_path: meta.yaml missing required 'author' field"
    has_error=1
  fi
  
  # Check recommended fields
  if ! grep -q "^description:" "$meta_file"; then
    warn "$resource_path: meta.yaml missing 'description' field (recommended)"
  fi
  
  # Validate YAML syntax (basic check)
  if command -v python3 &> /dev/null; then
    if ! python3 -c "import yaml; yaml.safe_load(open('$meta_file'))" 2>/dev/null; then
      error "$resource_path: meta.yaml has invalid YAML syntax"
      has_error=1
    fi
  fi
  
  return $has_error
}

validate_resource() {
  local resource_dir="$1"
  local type_name="$2"
  local resource_name=$(basename "$resource_dir")
  
  # Skip templates
  if [[ "$resource_name" == _* ]] || [[ "$resource_name" == .* ]]; then
    info "Skipping template: $resource_name"
    return 0
  fi
  
  echo ""
  echo "📦 $type_name/$resource_name"
  
  # Check naming convention
  if [[ ! "$resource_name" =~ ^[a-z0-9]+(-[a-z0-9]+)*$ ]]; then
    warn "Name should be kebab-case (lowercase with hyphens)"
  fi
  
  # Track for conflict detection
  local name_key="${type_name}:${resource_name,,}"  # lowercase
  if [[ -n "${ALL_NAMES[$name_key]}" ]]; then
    error "Naming conflict with: ${ALL_NAMES[$name_key]}"
  else
    ALL_NAMES[$name_key]="$type_name/$resource_name"
  fi
  
  # Validate meta.yaml
  validate_meta_yaml "$resource_dir/meta.yaml" "$type_name/$resource_name"
  
  # Check for content file
  local md_files=($(find "$resource_dir" -maxdepth 1 -name "*.md" -type f))
  local md_count=${#md_files[@]}
  
  if [ "$md_count" -eq 0 ]; then
    error "No .md content file found"
  elif [ "$md_count" -gt 1 ]; then
    warn "Multiple .md files found: ${md_files[*]##*/}"
  else
    ok "Content file: ${md_files[0]##*/}"
  fi
  
  # Check for unexpected files
  local expected_pattern="^(meta\.yaml|.*\.md|README\.md)$"
  for file in "$resource_dir"/*; do
    [ -f "$file" ] || continue
    local filename=$(basename "$file")
    if [[ ! "$filename" =~ $expected_pattern ]]; then
      warn "Unexpected file: $filename"
    fi
  done
}

# Validate directory structure
for type_dir in "$RESOURCES_DIR"/*/; do
  [ -d "$type_dir" ] || continue
  
  type_name=$(basename "$type_dir")
  
  # Check if valid type
  if [[ ! " ${VALID_TYPES[*]} " =~ " ${type_name} " ]]; then
    error "Invalid resource type: $type_name"
    info "Valid types: ${VALID_TYPES[*]}"
    continue
  fi
  
  echo ""
  echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
  echo "📁 $type_name/"
  echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
  
  # Validate each resource
  for resource_dir in "$type_dir"*/; do
    [ -d "$resource_dir" ] || continue
    validate_resource "$resource_dir" "$type_name"
  done
done

# Summary
echo ""
echo ""
echo "═══════════════════════════════════════════════════════"
echo "  Summary"
echo "═══════════════════════════════════════════════════════"
echo ""
echo "  Errors:   $ERRORS"
echo "  Warnings: $WARNINGS"
echo ""

if [ "$ERRORS" -gt 0 ]; then
  echo -e "${RED}❌ VALIDATION FAILED${NC}"
  echo ""
  echo "Please fix the errors above before submitting your PR."
  exit 1
elif [ "$WARNINGS" -gt 0 ]; then
  echo -e "${YELLOW}⚠️  VALIDATION PASSED WITH WARNINGS${NC}"
  echo ""
  echo "Consider addressing the warnings above."
  exit 0
else
  echo -e "${GREEN}✅ VALIDATION PASSED${NC}"
  exit 0
fi
