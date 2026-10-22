cask "sheaf" do
  version "0.1.0"
  sha256 "1f66aa92c1f03f53f3e7778d9cd4042df0a7a08d71627ed12c04c3c4a49b2853"

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
