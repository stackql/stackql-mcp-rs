//! sre-agent-sidecar: an SRE assurance agent on an embedded StackQL MCP server.
//! Act 3, the sidecar demo (slide "SIDECAR DETAILED").
//!
//! The four parts every agent has, in the order they appear below:
//!   model    Claude, through rig's anthropic provider (ANTHROPIC_API_KEY)
//!   tools    the StackQL MCP tools, served by a stackql binary the crate
//!            downloads, sha256-verifies and caches on first run (the
//!            sidecar), launched over stdio in read_only mode
//!   prompts  prompts/system.md (persona and StackQL context) and
//!            prompts/task.md (the sweep), compiled in with include_str!,
//!            placeholders filled from the environment
//!   loop     rig's: stream the turns, echo each tool call, print the report
//!
//! Setup:
//!   set -a; . ./.env; set +a     # ANTHROPIC_API_KEY, AWS_*, CLOUDFLARE_*, DEMO_*
//!   cargo run --release -p sre-agent-sidecar
//!
//! Tool calls go to stderr as they happen; the report goes to stdout.

use std::io::Write as _;

use anyhow::{bail, Context, Result};
use clap::Parser;
use futures::StreamExt;
use rig::agent::MultiTurnStreamItem;
use rig::client::{CompletionClient, ProviderClient};
use rig::providers::anthropic;
use rig::streaming::{StreamedAssistantContent, StreamingPrompt};
use rmcp::model::CallToolRequestParams;
use stackql_mcp::{Mode, StackqlMcp};

const SYSTEM: &str = include_str!("../prompts/system.md");
const TASK: &str = include_str!("../prompts/task.md");
const PROVIDERS: &[&str] = &["aws", "cloudflare"];

#[derive(Parser)]
#[command(
    name = "sre-agent-sidecar",
    version,
    about = "SRE assurance agent: Claude + the StackQL MCP server embedded as a sidecar"
)]
struct Cli {
    /// Ask this instead of running the compiled-in sweep (prompts/task.md).
    prompt: Option<String>,
    /// Claude model to use.
    #[arg(long, env = "SRE_AGENT_MODEL", default_value = "claude-opus-5")]
    model: String,
    /// Max tool-call rounds for one answer.
    #[arg(long, default_value_t = 40)]
    max_turns: usize,
    /// Pre-flight: start the server, pull the providers, list the tools and
    /// exit. No model call, no ANTHROPIC_API_KEY needed.
    #[arg(long)]
    check: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // The server. Sidecar: the stackql release pinned in the crate is
    // downloaded, verified and cached under ~/.stackql/mcp-server-bin on the
    // first run; every later start is offline. read_only: mutations are
    // refused by the server itself, whatever the model asks for. Provider
    // credentials (AWS_*, CLOUDFLARE_API_TOKEN) come from the environment.
    eprintln!("sre-agent-sidecar: starting the embedded StackQL MCP server (sidecar, read_only)");
    let server = StackqlMcp::builder()
        .mode(Mode::ReadOnly)
        .start()
        .await
        .context("starting the embedded StackQL MCP server")?;
    eprintln!(
        "sre-agent-sidecar: server binary {}",
        server.binary_path().display()
    );
    for provider in PROVIDERS {
        pull_provider(&server, provider).await?;
    }

    // The tools: whatever the server advertises, as rmcp hands them back.
    let tools = server.list_all_tools().await.context("listing MCP tools")?;
    eprintln!("connected: {} stackql tools (read_only)\n", tools.len());
    if cli.check {
        for tool in &tools {
            println!("  {}", tool.name);
        }
        server.shutdown().await.ok();
        return Ok(());
    }

    // The prompts: markdown compiled into the binary; {{ NAME }} from .env.
    let task = match cli.prompt {
        Some(prompt) => prompt,
        None => render(TASK, |name| std::env::var(name).ok())?,
    };

    // The model and the agent. rig consumes the rmcp tools and peer as-is.
    let agent = anthropic::Client::from_env()
        .context("ANTHROPIC_API_KEY not set")?
        .agent(&cli.model)
        .preamble(SYSTEM)
        .rmcp_tools(tools, server.peer().clone())
        .max_tokens(16_000)
        .default_max_turns(cli.max_turns)
        .build();
    eprintln!("sre-agent-sidecar: model {}\n", cli.model);

    // The loop: rig runs the tool-call rounds; we stream the text to stdout
    // and announce each tool call on stderr so the room sees the SQL.
    let mut stream = agent.stream_prompt(task).max_turns(cli.max_turns).await;
    let mut out = std::io::stdout();
    while let Some(item) = stream.next().await {
        match item? {
            MultiTurnStreamItem::StreamAssistantItem(StreamedAssistantContent::Text(text)) => {
                write!(out, "{}", text.text)?;
                out.flush()?;
            }
            MultiTurnStreamItem::StreamAssistantItem(StreamedAssistantContent::ToolCall {
                tool_call,
                ..
            }) => {
                eprintln!(
                    "  -> {} {}",
                    tool_call.function.name,
                    truncate(&tool_call.function.arguments.to_string(), 160)
                );
            }
            _ => {}
        }
    }
    println!();

    server.shutdown().await.ok();
    Ok(())
}

/// Pull a provider into the server's local cache through the MCP tool, so
/// the first query does not pay for it. Idempotent.
async fn pull_provider(server: &stackql_mcp::RunningServer, provider: &str) -> Result<()> {
    let mut params = CallToolRequestParams::new("pull_provider");
    params.arguments = serde_json::json!({ "provider": provider })
        .as_object()
        .cloned();
    server
        .call_tool(params)
        .await
        .with_context(|| format!("pulling provider {provider}"))?;
    Ok(())
}

/// Fill `{{ NAME }}` placeholders. A placeholder with no value is an error:
/// a sweep with a blank zone id or region would send the agent off guessing.
fn render(template: &str, lookup: impl Fn(&str) -> Option<String>) -> Result<String> {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    let mut missing = Vec::new();
    while let Some(start) = rest.find("{{") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let Some(end) = after.find("}}") else {
            out.push_str(&rest[start..]);
            rest = "";
            break;
        };
        let name = after[..end].trim();
        match lookup(name).filter(|v| !v.is_empty()) {
            Some(value) => out.push_str(&value),
            None => missing.push(name.to_string()),
        }
        rest = &after[end + 2..];
    }
    out.push_str(rest);
    if !missing.is_empty() {
        bail!(
            "prompts/task.md needs these set in the environment (.env): {}",
            missing.join(", ")
        );
    }
    Ok(out)
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max).collect();
    out.push_str(" ...");
    out
}
