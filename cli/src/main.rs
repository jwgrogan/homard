use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "arcctl", about = "Agent Run Control")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Execute a scheduled job
    RunJob {
        /// Job ID from ~/.arcctl/schedules/
        job_id: String,
    },
    /// Show system status
    Status,
    /// Switch Claude profile
    Switch {
        /// Profile name
        profile: String,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::RunJob { job_id }) => {
            let dirs = arcctl_core::config::ArcctlDirs::default_path();
            dirs.ensure_all().map_err(|e| anyhow::anyhow!(e))?;
            let mut executor = arcctl_core::executor::JobExecutor::new(dirs, &job_id)?;
            executor.execute().await?;
        }
        Some(Commands::Status) => {
            tracing::info!("Status check");
            // TODO: implement
        }
        Some(Commands::Switch { profile }) => {
            tracing::info!("Switching to profile: {}", profile);
            // TODO: implement
        }
        None => {
            println!("Launching arcctl GUI...");
            // TODO: launch Tauri app
        }
    }

    Ok(())
}
