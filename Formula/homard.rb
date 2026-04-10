class Homard < Formula
  desc "Lightweight personal AI assistant — always-on daemon with Telegram remote"
  homepage "https://github.com/jwgrogan/homard"
  url "https://github.com/jwgrogan/homard/archive/refs/heads/main.tar.gz"
  version "0.1.0"
  license "MIT"

  depends_on "rust" => :build
  depends_on "node" # For codex/claude CLI installation

  def install
    system "cargo", "build", "--release", "-p", "homard-cli"
    bin.install "target/release/homard"

    # Install default identity files
    (share/"homard/defaults").install Dir["defaults/*"]
  end

  def post_install
    ohai "Run 'homard setup' to configure your AI provider and Telegram"
  end

  def caveats
    <<~EOS
      🦞 Homard — your personal crustacean

      Quick start:
        homard setup          # Interactive setup wizard
        homard serve          # Start the daemon
        homard chat           # Chat from terminal
        homard install        # Enable always-on (launchd)

      Telegram remote access:
        1. Create a bot at https://t.me/BotFather
        2. Run 'homard setup' and enter the token
        3. Message your bot from anywhere

      Requires one of:
        npm install -g @openai/codex       # ChatGPT Plus/Pro
        npm install -g @anthropic-ai/claude-code  # Claude Pro/Max
    EOS
  end

  service do
    run [opt_bin/"homard", "serve"]
    keep_alive true
    log_path var/"log/homard.log"
    error_log_path var/"log/homard.log"
    environment_variables PATH: "/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin"
  end

  test do
    assert_match "Homard", shell_output("#{bin}/homard --help")
  end
end
