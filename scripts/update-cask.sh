#!/bin/bash
#
# update-cask.sh - Homebrew Caskファイルを更新
#
# 概要:
#   最新のGitHubリリースからSHA256を取得し、Caskファイルを更新する
#
# 使い方:
#   ./scripts/update-cask.sh          # 最新リリースで更新
#   ./scripts/update-cask.sh 0.7.0    # 指定バージョンで更新
#
set -e

if [ "$1" = "-h" ] || [ "$1" = "--help" ]; then
  echo "Usage: $0 [VERSION]"
  echo ""
  echo "Homebrew Caskファイルを更新"
  echo ""
  echo "引数:"
  echo "  VERSION    バージョン番号 (例: 0.7.0、省略時は最新リリース)"
  exit 0
fi

# バージョン取得
if [ -n "$1" ]; then
  VERSION="$1"
else
  TAG=$(gh release list --limit=1 --json tagName --jq '.[0].tagName')
  VERSION="${TAG#v}"
fi

TAG="v$VERSION"
echo "Updating Cask to $TAG..."

RELEASE_URL="https://github.com/naofumi-fujii/banzai/releases/download/$TAG/Banzai-$TAG.zip"
SHA256=$(curl -sL "$RELEASE_URL" | shasum -a 256 | cut -d' ' -f1)

if [ -z "$SHA256" ] || [ "$SHA256" = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855" ]; then
  echo "Error: SHA256の取得に失敗しました"
  echo "リリースが完了しているか確認してください: https://github.com/naofumi-fujii/banzai/releases"
  exit 1
fi

echo "SHA256: $SHA256"

sed -i '' "s/^  version \".*\"/  version \"$VERSION\"/" Casks/banzai.rb
sed -i '' "s/^  sha256 \".*\"/  sha256 \"$SHA256\"/" Casks/banzai.rb

git add Casks/banzai.rb
git commit -m "Cask: バージョンを${VERSION}に更新"
git push

echo "Done!"
