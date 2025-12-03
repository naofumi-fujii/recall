cask "banzai" do
  version "0.2.0"
  sha256 "1871d5fd47b00111a99f5c9f31ff19bc059b2a58a2028e25bdf5af54d4059600"

  url "https://github.com/naofumi-fujii/banzai/releases/download/v#{version}/Banzai-v#{version}.zip"
  name "Banzai"
  desc "macOS menu bar clipboard history monitor"
  homepage "https://github.com/naofumi-fujii/banzai"

  app "Banzai.app"

  zap trash: [
    "~/Library/Application Support/banzai",
  ]
end
