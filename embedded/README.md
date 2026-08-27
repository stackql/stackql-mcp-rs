# Act 3 - embedded MCP in a Rust application

Two agents, one crate. Both embed the StackQL MCP server via [`stackql-mcp`](https://crates.io/crates/stackql-mcp), hand the connected [`rmcp`](https://crates.io/crates/rmcp) client to a [rig](https://docs.rs/rig-core) agent, and run a fixed task written in markdown. They differ in how the server arrives and which model drives them; everything else is the same shape, on purpose.

| Program | Server | Model | Task |
|---|---|---|---|
| `sre-agent-sidecar/` | sidecar: downloaded, sha256-verified and cached on first run | Claude (`ANTHROPIC_API_KEY`, `SRE_AGENT_MODEL`, default `claude-opus-5`) | the morning assurance sweep over the service footprint in AWS and Cloudflare: health, exposure, edge, governance |
| `finops-agent-vendored/` | vendored: fetched at build time, compiled into the binary, extracted on first run | GPT-5 (`OPENAI_API_KEY`, `FINOPS_AGENT_MODEL`, default `gpt-5`) | the month-to-date FinOps report: spend by service from Cost Explorer, tagging gaps, waste |

Each program is one `src/main.rs` and two prompt files, `prompts/system.md` (persona and StackQL context) and `prompts/task.md` (the job), compiled in with `include_str!` and rendered with `{{ NAME }}` values from the environment. Both servers run in `Mode::ReadOnly`: the agents investigate and report, they cannot change anything, whatever the prompt.

## Run

```sh
set -a; . ./.env; set +a          # ANTHROPIC_API_KEY, OPENAI_API_KEY, AWS_*, CLOUDFLARE_*, DEMO_*
cd embedded
cargo build --release             # finops-agent-vendored's build.rs fetches the bundle once (network at build time)

./target/release/sre-agent-sidecar --check          # server, providers, tools; no model call
./target/release/sre-agent-sidecar                  # the sweep (prompts/task.md)
./target/release/sre-agent-sidecar "Which security groups in ap-southeast-2 allow port 22 from anywhere?"

# ~/.stackql/mcp-server-bin/0.10.605/

./target/release/finops-agent-vendored --check
./target/release/finops-agent-vendored               # the report
./target/release/finops-agent-vendored "What did EC2 cost us last month, by usage type?"
```

Tool calls are echoed on stderr as `-> tool {args}` while the answer streams to stdout, so the room sees every SQL statement the agent runs. `--max-turns` bounds the tool-call rounds (default 40; rig raises an error at the limit rather than returning a partial answer, so keep it above what the task needs). MSRV 1.88.

## The wiring

The whole integration is five lines. The crate hands back a connected rmcp client; rig's agent builder consumes its tools and peer directly.

```rust
let server = StackqlMcp::builder().mode(Mode::ReadOnly).start().await?;   // sidecar
let tools  = server.list_all_tools().await?;                               // the StackQL MCP tools
let agent  = anthropic::Client::from_env()?
    .agent("claude-opus-5")
    .preamble(include_str!("../prompts/system.md"))
    .rmcp_tools(tools, server.peer().clone())
    .build();
let mut stream = agent.stream_prompt(task).max_turns(40).await;           // the loop is rig's
```

The vendored agent differs in one builder line, `.bundle_bytes(stackql_mcp::include_bundle!())`, one Cargo feature (`stackql-mcp/vendored`) and a `build.rs` that fetches the bundle; and in the provider client, `openai::Client::from_env()?.agent("gpt-5")`.

Versions: `stackql-mcp` is pinned to the minor (`"0.10"`), because the crate version equals the stackql release it embeds. `rig-core` is `0.40`, the last release with the `rmcp` feature (0.41 dropped it); it and `stackql-mcp` resolve on the same `rmcp` 1.x, which is what lets the two plug together without an adapter.

## Sidecar vs vendored

| | Sidecar (default feature) | Vendored (`vendored` feature) |
|---|---|---|
| How the server arrives | the release pinned in the crate, `.mcpb` downloaded and cached on first run | fetched once at build time (`build.rs` calls `stackql_mcp::fetch_bundle()`), embedded with `include_bundle!()` |
| Integrity | sha256 checked against the pins rendered into the crate from the release's `.sha256` assets | the bytes are the artefact (fetched pin-verified at build time) |
| First run | download, verify, extract to `~/.stackql/mcp-server-bin/<version>/<platform>/`, launch | extract to `~/.stackql/mcp-server-bin/vendored/<hash>/`, launch |
| Network at run time | first run of each release only | never |
| Artefact size | small (the app) | app + ~40 MB (linux/windows) or ~90 MB (darwin-universal) |
| Choose when | you want small binaries and easy server updates | air-gapped hosts, single-file distribution, demos on conference wifi |

Overrides that work in both: `STACKQL_MCP_BIN` (run a stackql you already have), `STACKQL_MCP_BUNDLE` (extract a local `.mcpb`), `STACKQL_MCP_BUNDLE_FILE` at build time (embed a bundle you already have), `Builder::approot()`.

## Safety modes

`Mode::ReadOnly` (default) -> `Mode::Safe` -> `Mode::DeleteSafe` -> `Mode::FullAccess`. The mode is enforced inside the server: in `ReadOnly` a `run_mutation_query` is refused whatever the model asks; in `Safe` the server sends an MCP elicitation request before every write (deletes included); in `DeleteSafe` creates and updates run and only deletes ask; a client that cannot answer elicitation gets a refusal. Escalation is a caller opt-in via `.mode(...)`. Both agents here stay in `ReadOnly`; to act with a human in the loop, start the server in `Safe` with an rmcp `ClientHandler` that advertises elicitation and answers it at the terminal (`Builder::command()` gives you the acquired binary and canonical launch arguments to spawn it yourself).

## Prompts

The prompts are the product. `system.md` is the same shape in both agents: who the agent is, that the session is read-only, the MCP tools it has, how to pick the resource before writing SQL (discovery in two calls, flat one-row-per-item resources over JSON blobs, a tool-call budget, change approach rather than retry variants), how to write StackQL that lands on the first try (key columns in WHERE, plain-column filters, booleans as 0/1, JSON columns, the json_each CTE rule, fan-out, ORDER BY with LIMIT, the CTE for a third join), and how to report without narrating the debugging. Measured effect on the free-form question "which security groups allow port 22 from anywhere": 20 tool calls and a caveat section before, 7 tool calls and a clean table after. `task.md` is the job, numbered, naming the resources to read so the agent spends its tool calls on evidence rather than archaeology. Edit either file and rebuild; nothing else changes.
