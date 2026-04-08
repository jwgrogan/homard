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
    tracing_subscriber::fmt::init();
    let cli = Cli::parse();

    match cli.command {
        Commands::Serve => {
            let dirs = homard_core::config::HomardDirs::default_path();
            dirs.ensure_all()?;

            let config = homard_core::config::HomardConfig::load_or_default(&dirs.config_path());
            let store = Arc::new(tokio::sync::Mutex::new(
                homard_core::store::Store::open(&dirs.db_path())?,
            ));

            // Initialize OAuth manager
            let oauth = Arc::new(homard_core::llm::oauth::OAuthManager::new());
            // Load tokens from Keychain
            for provider_name in config.providers.keys() {
                let _ = oauth.load_from_keychain(provider_name).await;
            }

            // Initialize LLM client
            let llm = Arc::new(homard_core::llm::client::LlmClient::new(
                config.providers.clone(),
                config.active_provider.clone(),
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

            // Create API state
            let api_state = homard_core::api::AppState {
                agent: agent.clone(),
                store: store.clone(),
                config: Arc::new(tokio::sync::RwLock::new(config.clone())),
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

            // Start API server
            tracing::info!("Starting Homard daemon...");
            homard_core::api::serve(api_state, 17700).await
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
                            if let Some(response) =
                                data.get("response").and_then(|r| r.as_str())
                            {
                                println!("\n{}\n", response);
                            }
                        }
                        Err(e) => {
                            eprintln!(
                                "Error: {}. Is the daemon running? (homard serve)",
                                e
                            );
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
