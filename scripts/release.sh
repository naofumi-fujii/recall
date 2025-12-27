#!/bin/bash
# release.sh - Version update script (for CI)
#
# Usage:
#   ./scripts/release.sh patch   # 0.12.0 -> 0.12.1
#   ./scripts/release.sh minor   # 0.12.0 -> 0.13.0
#   ./scripts/release.sh major   # 0.12.0 -> 1.0.0
#
# Process:
#   1. Validate bump type
#   2. Calculate new version from current version
#   3. Check for duplicate tags
#   4. Update version in 3 files
#   5. Update Cargo.lock
#   6. Commit & create tag & push

set -euo pipefail

BUMP_TYPE="${1:-}"

if [[ -z "$BUMP_TYPE" ]]; then
  echo "❌ Please specify bump type"
  echo "Usage: ./scripts/release.sh [major|minor|patch]"
  exit 1
fi

# Validate bump type
if [[ ! "$BUMP_TYPE" =~ ^(major|minor|patch)$ ]]; then
  echo "❌ Invalid bump type (must be one of: major, minor, patch)"
  exit 1
fi

# Get current version
CURRENT=$(grep '^version' src-tauri/Cargo.toml | head -1 | sed 's/.*"\(.*\)"/\1/')
echo "Current version: $CURRENT"

# Parse version
IFS='.' read -r MAJOR MINOR PATCH <<< "$CURRENT"

# Calculate new version
case "$BUMP_TYPE" in
  major)
    NEW_MAJOR=$((MAJOR + 1))
    NEW_MINOR=0
    NEW_PATCH=0
    ;;
  minor)
    NEW_MAJOR=$MAJOR
    NEW_MINOR=$((MINOR + 1))
    NEW_PATCH=0
    ;;
  patch)
    NEW_MAJOR=$MAJOR
    NEW_MINOR=$MINOR
    NEW_PATCH=$((PATCH + 1))
    ;;
esac

VERSION="${NEW_MAJOR}.${NEW_MINOR}.${NEW_PATCH}"
echo "New version: $VERSION ($BUMP_TYPE)"

# Check for duplicate tag
TAG="v$VERSION"
if git rev-parse "$TAG" >/dev/null 2>&1; then
  echo "❌ Tag $TAG already exists"
  exit 1
fi

echo ""
echo "📝 Updating version..."

# Update Cargo.toml
sed -i '' "s/^version = \".*\"/version = \"$VERSION\"/" src-tauri/Cargo.toml

# Update package.json
jq ".version = \"$VERSION\"" package.json > package.json.tmp
mv package.json.tmp package.json

# Update tauri.conf.json
jq ".version = \"$VERSION\"" src-tauri/tauri.conf.json > tauri.conf.json.tmp
mv tauri.conf.json.tmp src-tauri/tauri.conf.json

echo "📦 Running cargo build..."
cd src-tauri && cargo build --quiet && cd ..

echo "📝 Committing changes..."
git config user.name "github-actions[bot]"
git config user.email "github-actions[bot]@users.noreply.github.com"
git add src-tauri/Cargo.toml src-tauri/Cargo.lock package.json src-tauri/tauri.conf.json
git commit -m "Update version to ${VERSION}"

echo "⬆️  Pushing commit..."
git push

echo "🏷️  Creating tag $TAG..."
git tag "$TAG"

echo "⬆️  Pushing tag $TAG..."
git push origin "$TAG"

echo ""
echo "✅ Version update complete: $TAG"

# Output version for GitHub Actions
if [[ -n "${GITHUB_OUTPUT:-}" ]]; then
  echo "version=$VERSION" >> "$GITHUB_OUTPUT"
fi
