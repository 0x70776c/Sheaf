cask "sheaf" do
  version "0.1.0"
  sha256 "1a7b154d8aad37b59dae9ecf06bc9741ef09c58d1c586db2994a86b9a68db9b6"

  url "https://github.com/0x70776c/Sheaf/releases/download/v#{version}/Sheaf-#{version}-universal.dmg"
  name "Sheaf"
  desc "Fast, 100% offline PDF toolkit for macOS"
  homepage "https://github.com/0x70776c/Sheaf"

  app "Sheaf.app"

  zap trash: [
    "~/Library/Application Support/app.sheaf.desktop",
    "~/Library/Caches/app.sheaf.desktop",
    "~/Library/Saved Application State/app.sheaf.desktop.savedState",
  ]
end
