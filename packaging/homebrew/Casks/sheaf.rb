cask "sheaf" do
  version "0.1.0"
  sha256 "730379c2eaecded9875ef89f354e30b883a4e4ebc066ac1a91c167550cfaf5bb"

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
