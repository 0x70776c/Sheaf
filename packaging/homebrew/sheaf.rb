class Sheaf < Formula
  desc "Fast, local PDF toolkit for macOS"
  homepage "https://github.com/0x70776c/Sheaf"
  version "0.1.0"
  license any_of: ["MIT", "Apache-2.0"]

  url "https://github.com/0x70776c/Sheaf/releases/download/v#{version}/Sheaf-cli-#{version}-universal.tar.gz"
  sha256 "edf4552275661781b308403d56832f0825d03b9be539394239bcd3cf36791b23"

  def install
    bin.install "bin/sheaf"
    lib.install "lib/libpdfium.dylib"
    bash_completion.install "completions/sheaf.bash" => "sheaf"
    zsh_completion.install "completions/_sheaf" => "_sheaf"
    fish_completion.install "completions/sheaf.fish" => "sheaf.fish"
  end

  test do
    system "#{bin}/sheaf", "--version"
  end
end
