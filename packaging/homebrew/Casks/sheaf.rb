cask "sheaf" do
  version "0.1.0"
  sha256 "70bbba4489eb28095b7130af63a7b357ab13d0964994b5940e3f8e6e16883718"

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
