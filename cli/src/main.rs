use clap::{Parser, Subcommand};
use std::sync::Arc;

#[derive(Parser)]
#[command(name = "homard", about = "Homard — your personal AI assistant")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Interactive setup wizard — configure provider, Telegram, and always-on
    Setup,
    /// Start the Homard daemon
    Serve,
    /// Chat with Homard (one-shot: homard chat -m "message")
    Chat {
        #[arg(short, long)]
        message: Option<String>,
    },
    /// Show daemon status
    Status,
    /// Stop the current run
    Stop,
    /// Install launchd plist for always-on daemon
    Install,
    /// Remove launchd plist
    Uninstall,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .init();
    eprintln!("[homard] starting...");
    let cli = Cli::parse();

    match cli.command {
        Commands::Setup => {
            setup_wizard().await?;
        }
        Commands::Serve => {
            eprintln!("[homard] serve command starting...");
            let dirs = homard_core::config::HomardDirs::default_path();
            dirs.ensure_all()?;
            eprintln!("[homard] dirs initialized");

            // Copy default identity files if not present
            let defaults_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../defaults");
            let identity_files = [
                "BOOTSTRAP.md",
                "SOUL.md",
                "IDENTITY.md",
                "USER.md",
                "AGENTS.md",
                "TOOLS.md",
                "HEARTBEAT.md",
                "MEMORY.md",
            ];
            for filename in &identity_files {
                let target = dirs.root().join(filename);
                if !target.exists() {
                    let source = defaults_dir.join(filename);
                    if source.exists() {
                        let _ = std::fs::copy(&source, &target);
                    }
                }
            }

            eprintln!("[homard] loading config...");
            let config = homard_core::config::HomardConfig::load_or_default(&dirs.config_path());
            eprintln!(
                "[homard] config loaded, active_provider={}",
                config.active_provider
            );
            let store = Arc::new(tokio::sync::Mutex::new(homard_core::store::Store::open(
                &dirs.db_path(),
            )?));

            // Clean up zombie "running" runs from previous daemon crash
            {
                let store_guard = store.lock().await;
                let _ = store_guard.cleanup_stale_runs();
            }

            eprintln!("[homard] store opened");
            // Initialize OAuth manager
            eprintln!("[homard] creating oauth manager...");
            let oauth = Arc::new(homard_core::llm::oauth::OAuthManager::new());
            eprintln!("[homard] oauth manager created, loading keychain tokens...");
            // Load tokens from Keychain
            for provider_name in config.providers.keys() {
                eprintln!("[homard] loading keychain for provider: {}", provider_name);
                match oauth.load_from_keychain(provider_name).await {
                    Ok(found) => eprintln!("[homard]   -> found: {}", found),
                    Err(e) => eprintln!("[homard]   -> error: {}", e),
                }
            }
            eprintln!("[homard] keychain loading complete");

            eprintln!("[homard] oauth initialized");
            // Shared config Arc for LLM client and AppState
            let shared_config = Arc::new(tokio::sync::RwLock::new(config.clone()));

            // Initialize LLM client (reads provider config from shared Arc, no disk reload)
            let llm = Arc::new(homard_core::llm::client::LlmClient::new(
                shared_config.clone(),
                oauth.clone(),
            ));

            // Initialize security manager
            let security = Arc::new(homard_core::security::SecurityManager::new(
                config.permission_level.clone(),
            ));

            // Initialize tool registry
            let mut tools = homard_core::tools::registry::ToolRegistry::new();
            // Register built-in tools
            tools.register(
                homard_core::tools::shell::schema(),
                homard_core::tools::shell::execute,
            );
            tools.register(
                homard_core::tools::web::search_schema(),
                homard_core::tools::web::search,
            );
            tools.register(
                homard_core::tools::web::fetch_schema(),
                homard_core::tools::web::fetch,
            );
            tools.register(
                homard_core::tools::files::read_schema(),
                homard_core::tools::files::read,
            );
            tools.register(
                homard_core::tools::files::write_schema(),
                homard_core::tools::files::write,
            );
            // Register shell tools from config
            tools.register_shell_tools(&config.shell_tools);
            // Register memory tools (need store reference)
            {
                let store_clone = store.clone();
                tools.register(homard_core::tools::memory::save_schema(), move |args| {
                    let s = store_clone.clone();
                    async move { homard_core::tools::memory::save(args, s).await }
                });
            }
            {
                let store_clone = store.clone();
                tools.register(homard_core::tools::memory::search_schema(), move |args| {
                    let s = store_clone.clone();
                    async move { homard_core::tools::memory::search(args, s).await }
                });
            }
            // Register user profile tool (needs homard dir)
            {
                let homard_dir = dirs.root().to_path_buf();
                tools.register(homard_core::tools::user_profile::schema(), move |args| {
                    let dir = homard_dir.clone();
                    async move { homard_core::tools::user_profile::execute(args, dir).await }
                });
            }
            // Register CLI session tools
            {
                let store_clone = store.clone();
                let pref_cli = config.preferred_coding_cli.clone();
                let fb_cli = config.coding_cli_fallback.clone();
                tools.register(homard_core::tools::session::spawn_schema(), move |args| {
                    let s = store_clone.clone();
                    let p = pref_cli.clone();
                    let f = fb_cli.clone();
                    async move { homard_core::tools::session::spawn(args, s, p, f).await }
                });
            }
            {
                let store_clone = store.clone();
                tools.register(
                    homard_core::tools::session::list_sessions_schema(),
                    move |args| {
                        let s = store_clone.clone();
                        async move { homard_core::tools::session::list(args, s).await }
                    },
                );
            }
            {
                let store_clone = store.clone();
                tools.register(
                    homard_core::tools::session::kill_session_schema(),
                    move |args| {
                        let s = store_clone.clone();
                        async move { homard_core::tools::session::kill(args, s).await }
                    },
                );
            }
            let tools = Arc::new(tools);

            // Stop signal
            let (stop_tx, stop_rx) = tokio::sync::watch::channel(false);

            // Initialize context builder
            let context =
                homard_core::agent::context::ContextBuilder::new(dirs.root().to_path_buf());

            // Initialize agent loop
            let agent = Arc::new(homard_core::agent::r#loop::AgentLoop::new(
                llm.clone(),
                tools.clone(),
                store.clone(),
                context,
                security.clone(),
                stop_rx,
            ));

            // Start Telegram poller
            let telegram_client = {
                #[cfg(target_os = "macos")]
                {
                    match homard_core::config::get_telegram_token(&dirs) {
                        Ok(Some(token)) => {
                            let client =
                                Arc::new(homard_core::telegram::TelegramClient::new(&token));
                            let poller_dirs = dirs.clone();
                            let poller_agent = agent.clone();
                            let poller_client = client.clone();
                            let poller_cancel = tokio_util::sync::CancellationToken::new();
                            let poller_stop = stop_tx.clone();
                            let cancel_clone = poller_cancel.clone();

                            let poller_security = security.clone();
                            let poller_config = shared_config.clone();

                            tokio::spawn(async move {
                                homard_core::telegram::poller::run_poller(
                                    poller_dirs,
                                    poller_agent,
                                    poller_client,
                                    cancel_clone,
                                    poller_stop,
                                    poller_security,
                                    poller_config,
                                )
                                .await;
                            });

                            Some(client)
                        }
                        _ => None,
                    }
                }
                #[cfg(not(target_os = "macos"))]
                {
                    None::<Arc<homard_core::telegram::TelegramClient>>
                }
            };

            // Start cron scheduler
            {
                let sched_dirs = dirs.clone();
                let sched_agent = agent.clone();
                let sched_store = store.clone();
                let sched_tg = telegram_client.clone();
                let sched_cancel = tokio_util::sync::CancellationToken::new();
                let cancel_clone = sched_cancel.clone();

                tokio::spawn(async move {
                    homard_core::scheduler::cron::run_scheduler(
                        sched_dirs,
                        sched_agent,
                        sched_store,
                        sched_tg,
                        cancel_clone,
                    )
                    .await;
                });
            }

            // Graceful shutdown handler
            let shutdown_stop = stop_tx.clone();
            tokio::spawn(async move {
                let mut sigterm =
                    tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                        .expect("failed to register SIGTERM handler");
                let mut sigint =
                    tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())
                        .expect("failed to register SIGINT handler");

                tokio::select! {
                    _ = sigterm.recv() => {
                        tracing::info!("Received SIGTERM, shutting down gracefully...");
                    }
                    _ = sigint.recv() => {
                        tracing::info!("Received SIGINT, shutting down gracefully...");
                    }
                }

                // Signal agent loop to stop
                let _ = shutdown_stop.send(true);
                tracing::info!(
                    "Shutdown signal sent. Daemon will stop after current work completes."
                );
            });

            // Create API state
            let api_state = homard_core::api::AppState {
                agent: agent.clone(),
                store: store.clone(),
                config: shared_config.clone(),
                security: security.clone(),
                oauth: oauth.clone(),
                homard_dir: dirs.root().to_path_buf(),
                stop_tx,
            };

            // Check for bootstrap
            if !config.bootstrapped {
                let bootstrap_path = dirs.root().join("BOOTSTRAP.md");
                if bootstrap_path.exists() {
                    tracing::info!("Running bootstrap...");
                    let _bootstrap_prompt = tokio::fs::read_to_string(&bootstrap_path)
                        .await
                        .unwrap_or_else(|_| {
                            "Introduce yourself and learn about the user.".to_string()
                        });
                    // Bootstrap will run when first message comes in
                }
            }

            // Pre-warm codex app-server if codex_cli is the active provider
            if config.active_provider == "codex_cli" {
                eprintln!("[homard] pre-warming codex app-server...");
                llm.warmup_codex().await;
            }

            // Start API server
            eprintln!("[homard] starting API server on :17700...");
            homard_core::api::serve(api_state, 17700)
                .await
                .map_err(|e| anyhow::anyhow!("{}", e))?;
        }
        Commands::Chat { message } => {
            let client = reqwest::Client::new();
            if let Some(msg) = message {
                // One-shot mode
                let resp = client
                    .post("http://127.0.0.1:17700/chat")
                    .json(&serde_json::json!({"message": msg}))
                    .send()
                    .await?;
                let data: serde_json::Value = resp.json().await?;
                if let Some(response) = data.get("response").and_then(|r| r.as_str()) {
                    println!("{}", response);
                }
            } else {
                // Interactive mode
                println!("Homard interactive chat (Ctrl+C to exit)");
                println!("---");
                let stdin = std::io::stdin();
                let mut input = String::new();
                loop {
                    print!("> ");
                    use std::io::Write;
                    std::io::stdout().flush()?;
                    input.clear();
                    if stdin.read_line(&mut input)? == 0 {
                        break;
                    }
                    let trimmed = input.trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    if trimmed == "/quit" || trimmed == "/exit" {
                        break;
                    }

                    match client
                        .post("http://127.0.0.1:17700/chat")
                        .json(&serde_json::json!({"message": trimmed}))
                        .send()
                        .await
                    {
                        Ok(resp) => {
                            let data: serde_json::Value = resp.json().await?;
                            if let Some(response) = data.get("response").and_then(|r| r.as_str()) {
                                println!("\n{}\n", response);
                            }
                        }
                        Err(e) => {
                            eprintln!("Error: {}. Is the daemon running? (homard serve)", e);
                        }
                    }
                }
            }
        }
        Commands::Status => {
            let client = reqwest::Client::new();
            match client.get("http://127.0.0.1:17700/status").send().await {
                Ok(resp) => {
                    let data: serde_json::Value = resp.json().await?;
                    println!("Homard daemon status:");
                    println!(
                        "  Running: {}",
                        data.get("running")
                            .and_then(|r| r.as_bool())
                            .unwrap_or(false)
                    );
                    println!(
                        "  Provider: {}",
                        data.get("active_provider")
                            .and_then(|p| p.as_str())
                            .unwrap_or("none")
                    );
                    println!(
                        "  Model: {}",
                        data.get("active_model")
                            .and_then(|m| m.as_str())
                            .unwrap_or("none")
                    );
                    println!(
                        "  Permission: {:?}",
                        data.get("permission_level")
                            .and_then(|p| p.as_str())
                            .unwrap_or("supervised")
                    );
                    println!(
                        "  Telegram: {}",
                        data.get("telegram_connected")
                            .and_then(|t| t.as_bool())
                            .unwrap_or(false)
                    );
                }
                Err(_) => {
                    println!("Homard daemon is not running. Start with: homard serve");
                }
            }
        }
        Commands::Stop => {
            let client = reqwest::Client::new();
            match client.post("http://127.0.0.1:17700/stop").send().await {
                Ok(_) => println!("Stop signal sent."),
                Err(_) => println!("Daemon not running."),
            }
        }
        Commands::Install => {
            println!("Installing launchd plist...");
            // Create a simple plist that runs `homard serve`
            let bin_path = homard_core::schedule::resolve_homard_bin()
                .unwrap_or_else(|_| "homard".to_string());
            let home = dirs::home_dir().expect("No home directory");
            let plist_content = format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.homard.daemon</string>
    <key>ProgramArguments</key>
    <array>
        <string>{}</string>
        <string>serve</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>{}/.homard/logs/daemon.stdout.log</string>
    <key>StandardErrorPath</key>
    <string>{}/.homard/logs/daemon.stderr.log</string>
</dict>
</plist>"#,
                bin_path,
                home.display(),
                home.display(),
            );

            let plist_path = home.join("Library/LaunchAgents/com.homard.daemon.plist");
            std::fs::create_dir_all(plist_path.parent().unwrap())?;
            std::fs::write(&plist_path, plist_content)?;

            // Load the plist
            let output = std::process::Command::new("launchctl")
                .args(["load", &plist_path.to_string_lossy()])
                .output()?;
            if output.status.success() {
                println!("Installed and started: {}", plist_path.display());
            } else {
                println!(
                    "Installed at {} but launchctl load failed",
                    plist_path.display()
                );
                println!("{}", String::from_utf8_lossy(&output.stderr));
            }
        }
        Commands::Uninstall => {
            let home = dirs::home_dir().expect("No home directory");
            let plist_path = home.join("Library/LaunchAgents/com.homard.daemon.plist");
            if plist_path.exists() {
                let _ = std::process::Command::new("launchctl")
                    .args(["unload", &plist_path.to_string_lossy()])
                    .output();
                std::fs::remove_file(&plist_path)?;
                println!("Uninstalled.");
            } else {
                println!("No launchd plist found.");
            }
        }
    }

    Ok(())
}

async fn setup_wizard() -> anyhow::Result<()> {
    use std::io::{self, BufRead, Write};

    let dirs = homard_core::config::HomardDirs::default_path();
    dirs.ensure_all()?;

    // Copy defaults
    let defaults_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../defaults");
    let identity_files = [
        "BOOTSTRAP.md",
        "SOUL.md",
        "IDENTITY.md",
        "USER.md",
        "AGENTS.md",
        "TOOLS.md",
        "HEARTBEAT.md",
        "MEMORY.md",
    ];
    for filename in &identity_files {
        let target = dirs.root().join(filename);
        if !target.exists() {
            let source = defaults_dir.join(filename);
            if source.exists() {
                let _ = std::fs::copy(&source, &target);
            }
        }
    }

    println!();
    println!("  🦞 Homard Setup");
    println!("  ───────────────");
    println!();

    let stdin = io::stdin();
    let mut input = String::new();

    // Step 1: Provider
    println!("  1. Choose your AI provider:");
    println!();
    let has_codex = std::process::Command::new("which")
        .arg("codex")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    let has_claude = std::process::Command::new("which")
        .arg("claude")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    println!(
        "     [1] Codex CLI {}  (ChatGPT Plus/Pro)",
        if has_codex {
            "✓ installed"
        } else {
            "— not installed"
        }
    );
    println!(
        "     [2] Claude CLI {} (Claude Pro/Max)",
        if has_claude {
            "✓ installed"
        } else {
            "— not installed"
        }
    );
    println!("     [3] Both");
    println!();
    print!("  > ");
    io::stdout().flush()?;
    input.clear();
    stdin.lock().read_line(&mut input)?;
    let choice = input.trim();

    let use_codex = choice == "1" || choice == "3" || choice.is_empty();
    let use_claude = choice == "2" || choice == "3";

    // Install missing CLIs
    if use_codex && !has_codex {
        println!();
        print!("  Codex CLI not found. Install it? [Y/n] ");
        io::stdout().flush()?;
        input.clear();
        stdin.lock().read_line(&mut input)?;
        if !input.trim().eq_ignore_ascii_case("n") {
            println!("  Installing Codex CLI...");
            let status = std::process::Command::new("npm")
                .args(["install", "-g", "@openai/codex"])
                .status();
            match status {
                Ok(s) if s.success() => println!("  ✓ Codex CLI installed"),
                _ => println!("  ✗ Failed — run manually: npm install -g @openai/codex"),
            }
        }
    }

    if use_claude && !has_claude {
        println!();
        print!("  Claude CLI not found. Install it? [Y/n] ");
        io::stdout().flush()?;
        input.clear();
        stdin.lock().read_line(&mut input)?;
        if !input.trim().eq_ignore_ascii_case("n") {
            println!("  Installing Claude CLI...");
            let status = std::process::Command::new("npm")
                .args(["install", "-g", "@anthropic-ai/claude-code"])
                .status();
            match status {
                Ok(s) if s.success() => println!("  ✓ Claude CLI installed"),
                _ => {
                    println!("  ✗ Failed — run manually: npm install -g @anthropic-ai/claude-code")
                }
            }
        }
    }

    // Login to provider
    if use_codex {
        let logged_in = std::path::Path::new(
            &dirs::home_dir()
                .unwrap_or_default()
                .join(".codex/auth.json"),
        )
        .exists();
        if !logged_in {
            println!();
            println!("  Log in to Codex (uses your ChatGPT Plus/Pro account):");
            let _ = std::process::Command::new("codex").arg("login").status();
        } else {
            println!("  ✓ Codex already logged in");
        }
    }

    if use_claude {
        println!();
        println!("  Checking Claude login...");
        let status_output = std::process::Command::new("claude")
            .args(["auth", "status"])
            .output();
        let logged_in = status_output
            .map(|o| {
                let text = String::from_utf8_lossy(&o.stdout);
                text.contains("loggedIn\": true") || text.contains("\"loggedIn\":true")
            })
            .unwrap_or(false);

        if !logged_in {
            println!("  Log in to Claude (uses your Claude Pro/Max account):");
            let _ = std::process::Command::new("claude").arg("login").status();
        } else {
            println!("  ✓ Claude already logged in");
        }
    }

    // Save provider config
    let mut config = homard_core::config::HomardConfig::load_or_default(&dirs.config_path());
    if use_codex {
        config.providers.insert(
            "codex_cli".to_string(),
            homard_core::types::ProviderConfig {
                kind: homard_core::types::ProviderKind::CodexCli,
                auth_type: "cli".to_string(),
                model: "gpt-5.4".to_string(),
                client_id: None,
                token_keychain_ref: None,
                api_key_keychain_ref: None,
                base_url: None,
            },
        );
        config.active_provider = "codex_cli".to_string();
    }
    if use_claude {
        config.providers.insert(
            "claude_cli".to_string(),
            homard_core::types::ProviderConfig {
                kind: homard_core::types::ProviderKind::ClaudeCli,
                auth_type: "cli".to_string(),
                model: "claude-sonnet-4-6".to_string(),
                client_id: None,
                token_keychain_ref: None,
                api_key_keychain_ref: None,
                base_url: None,
            },
        );
        if !use_codex {
            config.active_provider = "claude_cli".to_string();
        }
    }

    // Step 2: Telegram
    println!();
    println!("  2. Telegram (remote access — optional):");
    println!("     Create a bot at https://t.me/BotFather");
    print!("     Bot token (or Enter to skip): ");
    io::stdout().flush()?;
    input.clear();
    stdin.lock().read_line(&mut input)?;
    let token = input.trim().to_string();

    if !token.is_empty() {
        let tg_client = homard_core::telegram::TelegramClient::new(&token);
        match tg_client.verify().await {
            Ok(bot_name) => {
                println!("  ✓ Bot @{} connected", bot_name);
                #[cfg(target_os = "macos")]
                {
                    let _ = homard_core::config::save_telegram_token(&dirs, &token);
                }
                // Reload config after token save
                config = homard_core::config::HomardConfig::load_or_default(&dirs.config_path());

                print!("     Your Telegram @username: ");
                io::stdout().flush()?;
                input.clear();
                stdin.lock().read_line(&mut input)?;
                let username = input.trim().trim_start_matches('@').to_string();
                if !username.is_empty() {
                    if !config.telegram.allowed_usernames.contains(&username) {
                        config.telegram.allowed_usernames.push(username.clone());
                    }
                    println!("  ✓ @{} added to allowlist", username);
                }
            }
            Err(e) => println!("  ✗ Invalid token: {}", e),
        }
    }

    // Step 3: Your name
    println!();
    print!("  3. Your first name: ");
    io::stdout().flush()?;
    input.clear();
    stdin.lock().read_line(&mut input)?;
    let name = input.trim().to_string();

    if !name.is_empty() {
        let user_md = format!("# User Profile\n\nName: {}\n", name);
        let _ = std::fs::write(dirs.root().join("USER.md"), user_md);
        println!("  ✓ Saved");
    }

    // Step 4: Always-on
    println!();
    print!("  4. Enable always-on (server mode)? [Y/n] ");
    io::stdout().flush()?;
    input.clear();
    stdin.lock().read_line(&mut input)?;

    if !input.trim().eq_ignore_ascii_case("n") {
        config.server_mode = homard_core::types::ServerMode::On;

        #[cfg(target_os = "macos")]
        {
            let bin_path = homard_core::schedule::resolve_homard_bin()
                .unwrap_or_else(|_| "homard".to_string());
            let home = dirs::home_dir().expect("home dir");
            let plist = format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key><string>com.homard.daemon</string>
    <key>ProgramArguments</key><array><string>{}</string><string>serve</string></array>
    <key>RunAtLoad</key><true/>
    <key>KeepAlive</key><true/>
    <key>ThrottleInterval</key><integer>10</integer>
    <key>StandardOutPath</key><string>{}/.homard/logs/daemon.stdout.log</string>
    <key>StandardErrorPath</key><string>{}/.homard/logs/daemon.stderr.log</string>
    <key>EnvironmentVariables</key><dict><key>PATH</key><string>/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin</string></dict>
</dict>
</plist>"#,
                bin_path,
                home.display(),
                home.display()
            );
            let plist_path = home.join("Library/LaunchAgents/com.homard.daemon.plist");
            std::fs::create_dir_all(plist_path.parent().unwrap())?;
            std::fs::write(&plist_path, plist)?;
            let uid = unsafe { libc::getuid() };
            let _ = std::process::Command::new("launchctl")
                .args([
                    "bootstrap",
                    &format!("gui/{}", uid),
                    &plist_path.to_string_lossy(),
                ])
                .output();
            println!("  ✓ Always-on enabled (launchd)");
        }

        #[cfg(target_os = "windows")]
        {
            println!("  ✓ Always-on: run `homard install` to set up Windows Task Scheduler");
        }
    }

    // Save config
    config.bootstrapped = true;
    config.save(&dirs.config_path())?;

    println!();
    println!("  ───────────────");
    println!("  🦞 Homard is ready!");
    println!();
    println!("  Chat:     homard chat");
    println!("  Daemon:   homard serve");
    println!("  Status:   homard status");
    if !token.is_empty() {
        println!("  Telegram: message your bot");
    }
    println!();

    Ok(())
}
