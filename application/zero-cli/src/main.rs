// ============================================================================
// ZERO CLI - Main Entry Point
// Rich TUI interface for Zero Agent platform
// ============================================================================

mod app;
mod client;
mod events;
mod ui;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "zero")]
#[command(about = "Zero Agent CLI - AI agent platform", long_about = None)]
#[command(version)]
struct Cli {
    /// Gateway HTTP port
    #[arg(long, default_value = "18791", global = true)]
    port: u16,

    /// Gateway host
    #[arg(long, default_value = "127.0.0.1", global = true)]
    host: String,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Start interactive chat with an agent
    Chat {
        /// Agent ID to chat with
        #[arg(default_value = "assistant")]
        agent: String,

        /// Conversation ID (auto-generated if not provided)
        #[arg(short, long)]
        conversation: Option<String>,
    },

    /// Send a single message to an agent and get response
    Invoke {
        /// Agent ID
        agent: String,

        /// Message to send
        message: String,

        /// Conversation ID (auto-generated if not provided)
        #[arg(short, long)]
        conversation: Option<String>,
    },

    /// List available agents
    Agents {
        /// Show detailed information
        #[arg(short, long)]
        verbose: bool,
    },

    /// Daemon management
    Daemon {
        #[command(subcommand)]
        action: DaemonAction,
    },

    /// Check gateway status
    Status,
}

#[derive(Subcommand)]
enum DaemonAction {
    /// Check if daemon is running
    Status,

    /// Show daemon info
    Info,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::WARN.into()),
        )
        .with_target(false)
        .init();

    let cli = Cli::parse();
    let gateway_url = format!("http://{}:{}", cli.host, cli.port);
    let ws_url = format!("ws://{}:{}", cli.host, cli.port - 1); // WS is typically port - 1

    match cli.command {
        Some(Commands::Chat { agent, conversation }) => {
            let conv_id = conversation.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
            app::run_chat_tui(&gateway_url, &ws_url, &agent, &conv_id).await?;
        }

        Some(Commands::Invoke {
            agent,
            message,
            conversation,
        }) => {
            let conv_id = conversation.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
            run_invoke(&gateway_url, &ws_url, &agent, &conv_id, &message).await?;
        }

        Some(Commands::Agents { verbose }) => {
            run_list_agents(&gateway_url, verbose).await?;
        }

        Some(Commands::Daemon { action }) => match action {
            DaemonAction::Status => {
                run_daemon_status(&gateway_url).await?;
            }
            DaemonAction::Info => {
                run_daemon_info(&gateway_url).await?;
            }
        },

        Some(Commands::Status) => {
            run_status(&gateway_url).await?;
        }

        None => {
            // Default: run interactive chat TUI with agent selector
            app::run_chat_tui(&gateway_url, &ws_url, "assistant", &uuid::Uuid::new_v4().to_string()).await?;
        }
    }

    Ok(())
}

async fn run_invoke(
    gateway_url: &str,
    ws_url: &str,
    agent_id: &str,
    conversation_id: &str,
    message: &str,
) -> Result<()> {
    let client = client::GatewayClient::new(gateway_url, ws_url);

    // Check if gateway is running
    if !client.is_running().await {
        eprintln!("Error: Gateway daemon is not running");
        eprintln!("Start it with: cargo run -p daemon");
        std::process::exit(1);
    }

    println!("Invoking agent '{}'...\n", agent_id);

    // Connect and invoke
    let mut stream = client.invoke(agent_id, conversation_id, message).await?;

    // Stream the response
    while let Some(event) = stream.recv().await {
        match event {
            client::GatewayEvent::Token { content } => {
                print!("{}", content);
                std::io::Write::flush(&mut std::io::stdout())?;
            }
            client::GatewayEvent::Thinking { content } => {
                println!("\n[Thinking: {}]", content);
            }
            client::GatewayEvent::ToolCall { tool, .. } => {
                println!("\n[Tool: {}]", tool);
            }
            client::GatewayEvent::ToolResult { result, error, .. } => {
                if let Some(err) = error {
                    println!("[Tool Error: {}]", err);
                } else if let Some(res) = result {
                    let preview = if res.len() > 100 {
                        format!("{}...", &res[..100])
                    } else {
                        res
                    };
                    println!("[Tool Result: {}]", preview);
                }
            }
            client::GatewayEvent::Done { .. } => {
                println!("\n");
                break;
            }
            client::GatewayEvent::Error { message, .. } => {
                eprintln!("\nError: {}", message);
                break;
            }
            _ => {}
        }
    }

    Ok(())
}

async fn run_list_agents(gateway_url: &str, verbose: bool) -> Result<()> {
    let client = client::GatewayClient::new(gateway_url, "");

    if !client.is_running().await {
        eprintln!("Error: Gateway daemon is not running");
        std::process::exit(1);
    }

    let agents = client.list_agents().await?;

    if agents.is_empty() {
        println!("No agents found");
        return Ok(());
    }

    println!("Available agents:\n");
    for agent in agents {
        if verbose {
            println!("  {} - {}", agent.id, agent.name);
            if let Some(desc) = agent.description {
                println!("    {}", desc);
            }
            println!();
        } else {
            println!("  {}", agent.id);
        }
    }

    Ok(())
}

async fn run_daemon_status(gateway_url: &str) -> Result<()> {
    let client = client::GatewayClient::new(gateway_url, "");

    if client.is_running().await {
        println!("Daemon: Running");
        println!("URL: {}", gateway_url);
    } else {
        println!("Daemon: Not running");
        println!("Start with: cargo run -p daemon");
    }

    Ok(())
}

async fn run_daemon_info(gateway_url: &str) -> Result<()> {
    let client = client::GatewayClient::new(gateway_url, "");

    match client.get_status().await {
        Ok(status) => {
            println!("Gateway Status");
            println!("==============");
            println!("Status: {}", status.status);
            println!("Version: {}", status.version);
            if let Some(agents) = status.agent_count {
                println!("Agents: {}", agents);
            }
        }
        Err(_) => {
            println!("Daemon: Not running");
        }
    }

    Ok(())
}

async fn run_status(gateway_url: &str) -> Result<()> {
    run_daemon_info(gateway_url).await
}
