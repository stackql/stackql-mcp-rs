//! steward - a platform-engineering agent that keeps repositories on the
//! golden path. Act 3, step 3 (slide "DEMO").
//!
//! An embedded StackQL MCP server (sidecar by default, vendored straight into
//! this binary with `--features vendored`) wired into a rig agent. The
//! agent's tools are the StackQL MCP tools; the policy is a markdown file.
//!
//! - `steward check`  reads actual state, reports drift, proposes SQL fixes
//!   (server in read_only mode: writes are refused whatever the model does)
//! - `steward fix`    same, then remediates; the server runs in safe mode and
//!   asks for approval before every write, which this binary puts to the
//!   human at the terminal
//! - `steward ask`    a free-form read-only question
//! - `steward sql`    run one statement through the embedded server directly
//!   (no model): the deterministic fallback, and the quickest way to watch
//!   the approval prompt fire (`--write`)
//! - `steward preflight`  start the server, list tools, no model calls
//!
//! Needs ANTHROPIC_API_KEY for anything that calls the model. Tool-call
//! traces and diagnostics go to stderr; the answer goes to stdout.

mod embed;
mod prompt;

use std::io::Write as _;
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use futures::StreamExt;
use rig::agent::{Agent, MultiTurnStreamItem};
use rig::client::{CompletionClient, ProviderClient};
use rig::providers::anthropic;
use rig::providers::anthropic::completion::CompletionModel;
use rig::streaming::{StreamedAssistantContent, StreamingPrompt};
use rmcp::model::CallToolRequestParams;
use stackql_mcp::{Mode, StackqlMcp};
use tokio::io::AsyncBufReadExt as _;

const DEFAULT_MODEL: &str = "claude-opus-5";
/// The golden-path policy ships inside the binary, so the vendored build is
/// self-contained; --policy points at your own.
const DEFAULT_POLICY: &str = include_str!("../policies/golden-path.md");

#[derive(Parser)]
#[command(
    name = "steward",
    version,
    about = "Keeps repositories on the golden path: an agent over actual state, on an embedded StackQL MCP server"
)]
struct Cli {
    /// Repository to steward, as owner/name. Defaults to $GITHUB_ORG/$GITHUB_REPO.
    #[arg(long, global = true)]
    repo: Option<String>,
    /// Policy file (markdown) describing the golden path. Default: the
    /// golden-path policy compiled into the binary (policies/golden-path.md).
    #[arg(long, global = true)]
    policy: Option<PathBuf>,
    /// Provider to pull and query.
    #[arg(long, global = true, default_value = "github")]
    provider: String,
    /// Provider auth document as JSON. Default: STACKQL_GITHUB_USERNAME /
    /// STACKQL_GITHUB_PASSWORD if set (server-side default auth), else
    /// null_auth (public data, no credentials).
    #[arg(long, global = true)]
    auth: Option<String>,
    /// Model id (or set STEWARD_MODEL).
    #[arg(long, global = true, env = "STEWARD_MODEL", default_value = DEFAULT_MODEL)]
    model: String,
    /// Max agent tool-call rounds per task.
    #[arg(long, global = true, default_value_t = 24)]
    max_turns: usize,
    /// Approve every write without asking (scripted demos, CI). Decisions are still logged.
    #[arg(long, global = true)]
    yes: bool,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Read actual state, report drift against the policy, propose fixes (read-only)
    Check,
    /// Report drift and remediate it; every write is approved at the terminal
    Fix,
    /// Ask a free-form question about the estate (read-only)
    Ask { prompt: String },
    /// Interactive read-only session
    Chat,
    /// Run one SQL statement through the embedded server directly, no model.
    /// SELECT by default; --write sends a mutation (safe mode, approval prompt).
    Sql {
        sql: String,
        #[arg(long)]
        write: bool,
    },
    /// Start the embedded server and list its tools; no model calls
    Preflight,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let command = cli.command.unwrap_or(Command::Check);
    let can_write = matches!(command, Command::Fix | Command::Sql { write: true, .. });
    let mode = if can_write {
        Mode::Safe
    } else {
        Mode::ReadOnly
    };

    let repo = cli
        .repo
        .or_else(|| {
            Some(format!(
                "{}/{}",
                std::env::var("GITHUB_ORG").ok()?,
                std::env::var("GITHUB_REPO").ok()?
            ))
        })
        .context("pass --repo owner/name or set GITHUB_ORG and GITHUB_REPO")?;

    let auth: Option<serde_json::Value> = match &cli.auth {
        Some(raw) => Some(serde_json::from_str(raw).context("parsing --auth as JSON")?),
        None if cli.provider == "github" && github_creds_present() => None,
        None => Some(serde_json::json!({ &cli.provider: { "type": "null_auth" } })),
    };
    let auth_label = match &auth {
        None => "provider default (env)".to_string(),
        Some(a) => a[&cli.provider]["type"]
            .as_str()
            .unwrap_or("custom")
            .to_string(),
    };

    // 1. The embedded StackQL MCP server. Vendored when built with the
    //    feature, otherwise downloaded and sha256-verified on first run.
    let mut builder = StackqlMcp::builder().mode(mode);
    if let Some(a) = auth {
        builder = builder.auth(a);
    }
    #[cfg(feature = "vendored")]
    let builder = builder.bundle_bytes(stackql_mcp::include_bundle!());
    eprintln!(
        "steward: starting embedded StackQL MCP server ({})...",
        mode.as_str()
    );
    let server = embed::start(builder, embed::ApprovingClient::new(cli.yes)).await?;

    if let Err(e) = pull_provider(&server, &cli.provider).await {
        eprintln!(
            "steward: could not pull provider {} ({e:#}); continuing with whatever is cached",
            cli.provider
        );
    }
    let tools = server.list_all_tools().await.context("listing MCP tools")?;
    eprintln!(
        "steward: {} | provider {} ({}) | {} tools | mode {} | model {}",
        repo,
        cli.provider,
        auth_label,
        tools.len(),
        mode.as_str(),
        cli.model
    );

    if let Command::Sql { sql, write } = command {
        let tool = if write {
            "run_mutation_query"
        } else {
            "run_select_query"
        };
        eprintln!("  -> {tool}({sql})");
        let mut params = CallToolRequestParams::new(tool);
        params.arguments = serde_json::json!({ "sql": sql }).as_object().cloned();
        let result = server.call_tool(params).await?;
        for c in &result.content {
            if let Some(t) = c.as_text() {
                println!("{}", t.text);
            }
        }
        server.cancel().await.ok();
        return Ok(());
    }

    if let Command::Preflight = command {
        println!("MCP tools available to the agent:");
        for t in &tools {
            println!("  - {}", t.name);
        }
        let key = std::env::var("ANTHROPIC_API_KEY").is_ok();
        println!(
            "ANTHROPIC_API_KEY: {}",
            if key {
                "set"
            } else {
                "not set (needed for check/fix/ask/chat)"
            }
        );
        server.cancel().await.ok();
        return Ok(());
    }

    // 2. The policy and the agent. list_all_tools() + peer() off the embedded
    //    server go straight into rig's rmcp_tools(); that is the integration.
    let policy = match &cli.policy {
        Some(path) => std::fs::read_to_string(path)
            .with_context(|| format!("reading policy {}", path.display()))?,
        None => DEFAULT_POLICY.to_string(),
    };
    let agent = anthropic::Client::from_env()
        .context("ANTHROPIC_API_KEY not set")?
        .agent(&cli.model)
        .preamble(&prompt::system_prompt(&policy, &repo, can_write))
        .rmcp_tools(tools, server.peer().clone())
        .default_max_turns(cli.max_turns)
        .build();

    // 3. Run the task.
    let result = match command {
        Command::Check => {
            answer(&agent, &format!("Check {repo} against the policy and report drift."), cli.max_turns).await
        }
        Command::Fix => {
            answer(
                &agent,
                &format!("Check {repo} against the policy, then remediate every DRIFT finding the policy allows you to fix."),
                cli.max_turns,
            )
            .await
        }
        Command::Ask { prompt } => answer(&agent, &prompt, cli.max_turns).await,
        Command::Chat => repl(&agent, cli.max_turns).await,
        Command::Sql { .. } | Command::Preflight => unreachable!(),
    };

    server.cancel().await.ok();
    result
}

fn github_creds_present() -> bool {
    std::env::var("STACKQL_GITHUB_USERNAME").is_ok_and(|v| !v.is_empty())
        && std::env::var("STACKQL_GITHUB_PASSWORD").is_ok_and(|v| !v.is_empty())
}

/// Pull the provider into the server's local cache so the first query is
/// instant. Read-only safe and idempotent (offline if already cached).
async fn pull_provider(server: &embed::Server, provider: &str) -> Result<()> {
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

async fn repl(agent: &Agent<CompletionModel>, max_turns: usize) -> Result<()> {
    println!("steward chat. Ask about the estate; 'quit' or Ctrl-D to exit.\n");
    let mut lines = tokio::io::BufReader::new(tokio::io::stdin()).lines();
    loop {
        print!("you> ");
        std::io::stdout().flush().ok();
        let Some(line) = lines.next_line().await? else {
            break;
        };
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if matches!(line, "quit" | "exit" | ":q") {
            break;
        }
        if let Err(e) = answer(agent, line, max_turns).await {
            eprintln!("\nsteward: {e:#}");
        }
    }
    Ok(())
}

/// Stream one answer. Text goes to stdout as it arrives; every tool call the
/// agent makes is announced on stderr, so the room watches it query (and,
/// in fix mode, change) the live estate.
async fn answer(agent: &Agent<CompletionModel>, prompt: &str, max_turns: usize) -> Result<()> {
    let mut stream = agent.stream_prompt(prompt).multi_turn(max_turns).await;
    while let Some(item) = stream.next().await {
        match item? {
            MultiTurnStreamItem::StreamAssistantItem(StreamedAssistantContent::Text(text)) => {
                print!("{}", text.text);
                std::io::stdout().flush().ok();
            }
            MultiTurnStreamItem::StreamAssistantItem(StreamedAssistantContent::ToolCall {
                tool_call,
                ..
            }) => {
                let args = tool_call.function.arguments.to_string();
                eprintln!(
                    "\n  -> {}({})",
                    tool_call.function.name,
                    truncate(&args, 240)
                );
            }
            _ => {}
        }
    }
    println!();
    Ok(())
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max).collect();
    out.push_str(" ...");
    out
}
