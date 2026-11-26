# @story 12.1
# Homebrew Formula for Channel 1 Single-Binary CLI (`cowok-design`)
class CowokDesign < Formula
  desc "Single-binary Rust Design Harness CLI, Standalone Local Workbench & Open MCP Server"
  homepage "https://github.com/cowok-ai/cowok-design"
  version "1.0.0-rc.1"
  license "Apache-2.0"

  on_macos do
    on_arm do
      url "https://github.com/cowok-ai/cowok-design/releases/download/v#{version}/cowok-design-v#{version}-aarch64-apple-darwin.tar.gz"
      sha256 "REPLACE_WITH_RELEASE_SHA256_AARCH64_APPLE_DARWIN"
    end

    on_intel do
      url "https://github.com/cowok-ai/cowok-design/releases/download/v#{version}/cowok-design-v#{version}-x86_64-apple-darwin.tar.gz"
      sha256 "REPLACE_WITH_RELEASE_SHA256_X86_64_APPLE_DARWIN"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/cowok-ai/cowok-design/releases/download/v#{version}/cowok-design-v#{version}-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "REPLACE_WITH_RELEASE_SHA256_AARCH64_UNKNOWN_LINUX_GNU"
    end

    on_intel do
      url "https://github.com/cowok-ai/cowok-design/releases/download/v#{version}/cowok-design-v#{version}-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "REPLACE_WITH_RELEASE_SHA256_X86_64_UNKNOWN_LINUX_GNU"
    end
  end

  def install
    bin.install "cowok-design"
  end

  test do
    assert_match "cowok-design", shell_output("#{bin}/cowok-design --version")
    doctor_json = shell_output("#{bin}/cowok-design doctor --mode passive --json")
    assert_match "\"tools\"", doctor_json
  end
end
