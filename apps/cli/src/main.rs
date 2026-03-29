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

    /// Manage distillation
    Distill {
        #[command(subcommand)]
        action: DistillAction,
    },
}

#[derive(Subcommand)]
enum DaemonAction {
    /// Check if daemon is running
    Status,

    /// Show daemon info
    Info,
}

#[derive(Subcommand)]
enum DistillAction {
    /// Retroactively distill all undistilled sessions
    Backfill {
        /// Max concurrent distillation calls
        #[arg(long, default_value = "2")]
        concurrency: usize,
    },
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

        Some(Commands::Distill { action }) => match action {
            DistillAction::Backfill { concurrency } => {
                run_distill_backfill(&gateway_url, concurrency).await?;
            }
        },

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
                        format!("{}...", &res[..res.floor_char_boundary(100)])
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

// ============================================================================
// Distillation Backfill
// ============================================================================

#[derive(serde::Deserialize)]
struct UndistilledSession {
    session_id: String,
    #[allow(dead_code)]
    agent_id: String,
}

#[derive(serde::Deserialize)]
struct TriggerDistillationResponse {
    #[allow(dead_code)]
    session_id: String,
    status: String,
    facts_upserted: usize,
    error: Option<String>,
}

async fn run_distill_backfill(gateway_url: &str, _concurrency: usize) -> Result<()> {
    let client = reqwest::Client::new();

    // Check if gateway is running
    let health_resp = client
        .get(format!("{}/api/health", gateway_url))
        .timeout(std::time::Duration::from_secs(2))
        .send()
        .await;

    if health_resp.is_err() {
        eprintln!("Error: Gateway daemon is not running");
        eprintln!("Start it with: cargo run -p daemon");
        std::process::exit(1);
    }

    println!("Fetching undistilled sessions...");

    let sessions: Vec<UndistilledSession> = client
        .get(format!("{}/api/distillation/undistilled", gateway_url))
        .send()
        .await?
        .json()
        .await?;

    if sessions.is_empty() {
        println!("All sessions are already distilled. Nothing to do.");
        return Ok(());
    }

    println!("Found {} undistilled session(s)\n", sessions.len());

    let mut success_count = 0;
    let mut fail_count = 0;

    for (i, session) in sessions.iter().enumerate() {
        let short_id = if session.session_id.len() > 8 {
            &session.session_id[..8]
        } else {
            &session.session_id
        };

        print!("[{}/{}] Session {}... ", i + 1, sessions.len(), short_id);
        std::io::Write::flush(&mut std::io::stdout())?;

        let resp = client
            .post(format!(
                "{}/api/distillation/trigger/{}",
                gateway_url, session.session_id
            ))
            .send()
            .await;

        match resp {
            Ok(r) if r.status().is_success() => {
                match r.json::<TriggerDistillationResponse>().await {
                    Ok(body) => {
                        if body.status == "success" {
                            println!("ok ({} facts)", body.facts_upserted);
                            success_count += 1;
                        } else {
                            let err_msg = body.error.as_deref().unwrap_or("unknown error");
                            println!("failed ({})", err_msg);
                            fail_count += 1;
                        }
                    }
                    Err(e) => {
                        println!("failed (parse error: {})", e);
                        fail_count += 1;
                    }
                }
            }
            Ok(r) => {
                println!("failed (HTTP {})", r.status());
                fail_count += 1;
            }
            Err(e) => {
                println!("failed ({})", e);
                fail_count += 1;
            }
        }
    }

    println!();
    println!("Backfill complete: {} succeeded, {} failed", success_count, fail_count);

    Ok(())
}
