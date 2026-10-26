class Sheaf < Formula
  desc "Fast, 100% offline PDF toolkit for macOS"
  homepage "https://github.com/0x70776c/Sheaf"
  version "0.1.0"
  license any_of: ["MIT", "Apache-2.0"]

  url "https://github.com/0x70776c/Sheaf/releases/download/v#{version}/Sheaf-cli-#{version}-universal.tar.gz"
  sha256 "8b4c73e2c17d917e9b1a32c588e95225f34b506faddc8dec41dd583b4b9108bd"

  def install
    bin.install "sheaf"
    lib.install "lib/libpdfium.dylib"
    bash_completion.install "completions/sheaf.bash" => "sheaf"
    zsh_completion.install "completions/_sheaf" => "_sheaf"
    fish_completion.install "completions/sheaf.fish" => "sheaf.fish"
  end

  test do
    system "#{bin}/sheaf", "--version"
  end
end
