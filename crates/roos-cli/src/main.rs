use clap::{Parser, Subcommand};

mod cmd;

#[derive(Parser)]
#[command(name = "roos", version, about = "ROOS agent framework CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a new ROOS project
    New {
        /// Project name (used as directory and agent name)
        name: String,
    },
    /// Run an agent synchronously using roos.toml
    Run {
        /// Input text to send to the agent
        #[arg(short, long)]
        input: String,
        /// Path to roos.toml
        #[arg(short, long, default_value = "roos.toml")]
        config: String,
    },
    /// List agents defined in roos.toml
    List {
        /// Path to roos.toml
        #[arg(short, long, default_value = "roos.toml")]
        config: String,
    },
    /// Start ROOS in long-running server mode (trigger HTTP + scheduler)
    Serve {
        /// HTTP trigger port
        #[arg(short, long, default_value_t = 8080)]
        port: u16,
        /// Path to roos.toml
        #[arg(short, long, default_value = "roos.toml")]
        config: String,
        /// Fork to background — Linux/macOS only (not yet implemented)
        #[arg(long)]
        daemonize: bool,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::New { name } => cmd::new::run(&name),
        Commands::Run { input, config } => cmd::run::run(&config, &input).await,
        Commands::List { config } => cmd::list::run(&config),
        Commands::Serve {
            port,
            config,
            daemonize,
        } => cmd::serve::run(port, &config, daemonize).await,
    }
}
