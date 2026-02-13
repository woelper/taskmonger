#!/bin/bash
set -e

CARGO_TOML="Cargo.toml"

# Get current version
CURRENT=$(grep '^version' "$CARGO_TOML" | head -1 | sed 's/.*"\(.*\)"/\1/')
IFS='.' read -r MAJOR MINOR PATCH <<< "$CURRENT"

# Determine bump type (default: patch)
BUMP="${1:-patch}"
case "$BUMP" in
    major) MAJOR=$((MAJOR + 1)); MINOR=0; PATCH=0 ;;
    minor) MINOR=$((MINOR + 1)); PATCH=0 ;;
    patch) PATCH=$((PATCH + 1)) ;;
    *) echo "Usage: $0 [major|minor|patch]"; exit 1 ;;
esac

NEW_VERSION="${MAJOR}.${MINOR}.${PATCH}"
TAG="v${NEW_VERSION}"

echo "$CURRENT -> $NEW_VERSION"

# Update Cargo.toml
sed -i '' "s/^version = \"$CURRENT\"/version = \"$NEW_VERSION\"/" "$CARGO_TOML"

# Commit, tag, push
git add "$CARGO_TOML"
git commit -m "Bump version to $NEW_VERSION"
git tag "$TAG"
git push && git push --tags

echo "Released $TAG"
