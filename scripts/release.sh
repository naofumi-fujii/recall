#!/bin/bash
# release.sh - バージョン更新スクリプト (CI用)
#
# 使い方:
#   ./scripts/release.sh 0.12.0
#
# 処理内容:
#   1. バージョン形式のバリデーション
#   2. タグの重複チェック
#   3. 現在バージョンとの比較
#   4. 3ファイルのバージョン更新
#   5. Cargo.lock更新
#   6. コミット & タグ作成 & プッシュ

set -euo pipefail

VERSION="${1:-}"

if [[ -z "$VERSION" ]]; then
  echo "❌ バージョンを指定してください"
  echo "使い方: ./scripts/release.sh 0.12.0"
  exit 1
fi

# バージョン形式のバリデーション
if [[ ! "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "❌ 無効なバージョン形式です (例: 0.12.0)"
  exit 1
fi

# タグの重複チェック
TAG="v$VERSION"
if git rev-parse "$TAG" >/dev/null 2>&1; then
  echo "❌ タグ $TAG は既に存在します"
  exit 1
fi

# 現在のバージョンを取得
CURRENT=$(grep '^version' src-tauri/Cargo.toml | head -1 | sed 's/.*"\(.*\)"/\1/')
echo "現在のバージョン: $CURRENT"
echo "新しいバージョン: $VERSION"

# バージョン比較
IFS='.' read -r c1 c2 c3 <<< "$CURRENT"
IFS='.' read -r n1 n2 n3 <<< "$VERSION"

if [[ $n1 -lt $c1 ]] || \
   [[ $n1 -eq $c1 && $n2 -lt $c2 ]] || \
   [[ $n1 -eq $c1 && $n2 -eq $c2 && $n3 -le $c3 ]]; then
  echo "❌ 新しいバージョン ($VERSION) は現在のバージョン ($CURRENT) より大きい必要があります"
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
