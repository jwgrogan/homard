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
            tracing::info!("Running job: {}", job_id);
            // TODO: implement
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
