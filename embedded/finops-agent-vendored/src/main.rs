//! finops-agent-vendored: a FinOps analyst agent on an embedded StackQL MCP server.
//! Act 3, the vendored demo (slide "VENDORED DETAILED").
//!
//! The four parts every agent has, in the order they appear below:
//!   model    GPT-5, through rig's openai provider (OPENAI_API_KEY)
//!   tools    the StackQL MCP tools, served by a stackql binary that is
//!            compiled into this executable (build.rs fetches the bundle,
//!            include_bundle!() embeds it) and extracted on first run; no
//!            network needed to start, ever; read_only mode
//!   prompts  prompts/system.md (persona and StackQL context) and
//!            prompts/task.md (the report), compiled in with include_str!,
//!            placeholders filled from the environment and today's date
//!   loop     rig's: stream the turns, echo each tool call, print the report
//!
//! Setup:
//!   set -a; . ./.env; set +a     # OPENAI_API_KEY, AWS_*
//!   cargo run --release -p finops-agent-vendored
//!
//! Tool calls go to stderr as they happen; the report goes to stdout.

use std::io::Write as _;

use anyhow::{bail, Context, Result};
use clap::Parser;
use futures::StreamExt;
use rig::agent::MultiTurnStreamItem;
use rig::client::{CompletionClient, ProviderClient};
use rig::providers::openai;
use rig::streaming::{StreamedAssistantContent, StreamingPrompt};
use rmcp::model::CallToolRequestParams;
use stackql_mcp::{Mode, StackqlMcp};

const SYSTEM: &str = include_str!("../prompts/system.md");
const TASK: &str = include_str!("../prompts/task.md");
const PROVIDERS: &[&str] = &["aws"];

#[derive(Parser)]
#[command(
    name = "finops-agent-vendored",
    version,
    about = "FinOps analyst agent: GPT-5 + the StackQL MCP server vendored into the binary"
)]
struct Cli {
    /// Ask this instead of running the compiled-in report (prompts/task.md).
    prompt: Option<String>,
    /// OpenAI model to use.
    #[arg(long, env = "FINOPS_AGENT_MODEL", default_value = "gpt-5")]
    model: String,
    /// Max tool-call rounds for one answer.
    #[arg(long, default_value_t = 40)]
    max_turns: usize,
    /// Pre-flight: start the server, pull the provider, list the tools and
    /// exit. No model call, no OPENAI_API_KEY needed.
    #[arg(long)]
    check: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // The server. Vendored: the .mcpb bundle is part of this binary; the
    // crate extracts it under ~/.stackql/mcp-server-bin/vendored/<hash>/ on
    // the first run and launches it. This is the one line that differs from
    // the sidecar agent. read_only: mutations are refused by the server
    // itself. AWS credentials come from the environment.
    eprintln!("finops-agent-vendored: starting the embedded StackQL MCP server (vendored, read_only)");
    let server = StackqlMcp::builder()
        .mode(Mode::ReadOnly)
        .bundle_bytes(stackql_mcp::include_bundle!())
        .start()
        .await
        .context("starting the embedded StackQL MCP server")?;
    eprintln!(
        "finops-agent-vendored: server binary {}",
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

    // The prompts: markdown compiled into the binary; {{ NAME }} from .env,
    // {{ TODAY }} from the clock (Cost Explorer wants explicit dates).
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let task = match cli.prompt {
        Some(prompt) => prompt,
        None => render(TASK, |name| match name {
            "TODAY" => Some(today.clone()),
            _ => std::env::var(name).ok(),
        })?,
    };

    // The model and the agent. rig's openai client speaks the Responses API;
    // it consumes the rmcp tools and peer as-is.
    let agent = openai::Client::from_env()
        .context("OPENAI_API_KEY not set")?
        .agent(&cli.model)
        .preamble(SYSTEM)
        .rmcp_tools(tools, server.peer().clone())
        .max_tokens(16_000)
        .default_max_turns(cli.max_turns)
        .build();
    eprintln!("finops-agent-vendored: model {}\n", cli.model);

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
/// a report with a blank region would send the agent off guessing.
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
