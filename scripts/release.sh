#!/bin/bash
# release.sh - バージョン更新スクリプト (CI用)
#
# 使い方:
#   ./scripts/release.sh patch   # 0.12.0 -> 0.12.1
#   ./scripts/release.sh minor   # 0.12.0 -> 0.13.0
#   ./scripts/release.sh major   # 0.12.0 -> 1.0.0
#
# 処理内容:
#   1. bump typeのバリデーション
#   2. 現在バージョンから新バージョンを計算
#   3. タグの重複チェック
#   4. 3ファイルのバージョン更新
#   5. Cargo.lock更新
#   6. コミット & タグ作成 & プッシュ

set -euo pipefail

BUMP_TYPE="${1:-}"

if [[ -z "$BUMP_TYPE" ]]; then
  echo "❌ bump typeを指定してください"
  echo "使い方: ./scripts/release.sh [major|minor|patch]"
  exit 1
fi

# bump typeのバリデーション
if [[ ! "$BUMP_TYPE" =~ ^(major|minor|patch)$ ]]; then
  echo "❌ 無効なbump typeです (major, minor, patch のいずれかを指定)"
  exit 1
fi

# 現在のバージョンを取得
CURRENT=$(grep '^version' src-tauri/Cargo.toml | head -1 | sed 's/.*"\(.*\)"/\1/')
echo "現在のバージョン: $CURRENT"

# バージョンを分解
IFS='.' read -r MAJOR MINOR PATCH <<< "$CURRENT"

# 新バージョンを計算
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
echo "新しいバージョン: $VERSION ($BUMP_TYPE)"

# タグの重複チェック
TAG="v$VERSION"
if git rev-parse "$TAG" >/dev/null 2>&1; then
  echo "❌ タグ $TAG は既に存在します"
  exit 1
fi

echo ""
echo "📝 バージョンを更新中..."

# Cargo.toml を更新
sed -i '' "s/^version = \".*\"/version = \"$VERSION\"/" src-tauri/Cargo.toml

# package.json を更新
jq ".version = \"$VERSION\"" package.json > package.json.tmp
mv package.json.tmp package.json

# tauri.conf.json を更新
jq ".version = \"$VERSION\"" src-tauri/tauri.conf.json > tauri.conf.json.tmp
mv tauri.conf.json.tmp src-tauri/tauri.conf.json

echo "📦 cargo build を実行中..."
cd src-tauri && cargo build --quiet && cd ..

echo "📝 変更をコミット中..."
git config user.name "github-actions[bot]"
git config user.email "github-actions[bot]@users.noreply.github.com"
git add src-tauri/Cargo.toml src-tauri/Cargo.lock package.json src-tauri/tauri.conf.json
git commit -m "バージョンを${VERSION}に更新"

echo "⬆️  コミットをプッシュ中..."
git push

echo "🏷️  タグ $TAG を作成中..."
git tag "$TAG"

echo "⬆️  タグ $TAG をプッシュ中..."
git push origin "$TAG"

echo ""
echo "✅ バージョン更新完了: $TAG"

# GitHub Actions の output にバージョンを出力
if [[ -n "${GITHUB_OUTPUT:-}" ]]; then
  echo "version=$VERSION" >> "$GITHUB_OUTPUT"
fi
