#!/bin/bash
#
# release.sh - Banzaiのリリーススクリプト
#
# 概要:
#   Cargo.tomlからバージョンを読み取り、Gitタグを作成してプッシュする。
#   タグのプッシュにより、GitHub Actionsのリリースワークフローがトリガーされる。
#
# 使い方:
#   ./scripts/release.sh          # Cargo.tomlの現在のバージョンでリリース
#   ./scripts/release.sh 0.4.0    # バージョンを0.4.0に更新してリリース
#
# 前提条件:
#   - 対象バージョンのタグが存在しないこと
#   - mainブランチがリモートと同期していること
#
set -e

usage() {
  echo "Usage: $0 VERSION"
  echo ""
  echo "Banzaiのリリーススクリプト"
  echo ""
  echo "引数:"
  echo "  VERSION    新しいバージョン番号 (例: 0.4.0)"
  echo ""
  echo "例:"
  echo "  $0 0.4.0    # バージョンを0.4.0に更新してリリース"
  exit 0
}

if [ -z "$1" ] || [ "$1" = "-h" ] || [ "$1" = "--help" ]; then
  usage
fi

# Cargo.tomlのバージョンを更新
if [ -n "$1" ]; then
  NEW_VERSION="$1"
  echo "Updating version to $NEW_VERSION..."
  sed -i '' "s/^version = \".*\"/version = \"$NEW_VERSION\"/" Cargo.toml
  cargo build --quiet 2>/dev/null || cargo build
  git add Cargo.toml Cargo.lock
  git commit -m "バージョンを${NEW_VERSION}に更新"
fi

# Cargo.tomlからバージョンを取得
VERSION=$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)".*/\1/')
TAG="v$VERSION"

echo "Releasing $TAG..."

# 未コミットの変更がないか確認
if [ -n "$(git status --porcelain)" ]; then
  echo "Error: uncommitted changes exist"
  exit 1
fi

# タグが既に存在するか確認
if git rev-parse "$TAG" >/dev/null 2>&1; then
  echo "Error: tag $TAG already exists"
  exit 1
fi

# pushしてタグを作成
git push
git tag "$TAG"
git push origin "$TAG"

echo "Done! Release $TAG has been triggered."
echo "Check: https://github.com/naofumi-fujii/banzai/actions"
