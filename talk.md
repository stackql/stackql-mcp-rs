# Talk kit: Rust-native agentic platform engineering with embedded MCP

Presentation content and demo runbook for the meetup talk. Speaker is Jeff Aven (StackQL, OSS). The whole stack is MIT-licensed and public.

The submitted abstract:

> Title: Rust-native agentic platform engineering with embedded MCP
>
> Building Rust-native agents for platform engineering and observability, with the MCP server embedded and vendored straight into the Rust application. Demonstrated with concrete examples: stackql-agent, one binary that becomes a platform-engineering, SRE, or audit agent by swapping a system prompt, and auditron, a compliance copilot that ships as one self-contained binary with no runtime downloads.

The deck has six sections (a-f). Each section below lists the slides, the bullets that go on them, and speaker notes (the things to say out loud, not put on the slide). Demo runbook with exact commands and a fallback plan is at the end.

Style for slides: short bullets, one idea each. The prose here is the script, not the slide text.

## a) Intro to StackQL

### Slide a1: The problem

- Cloud and SaaS state lives behind dozens of APIs: AWS, GitHub, Google, Azure, Kubernetes, Okta, and on.
- Every one has its own SDK, auth, pagination, and quirks.
- Answering a flat question ("which repos have no branch protection", "what failed last night", "who has admin") means writing bespoke glue against each API.

Speaker notes: this is the daily reality of platform, SRE, and security teams. The data exists; getting at it uniformly is the tax.

### Slide a2: StackQL is cloud-as-SQL

- StackQL lets you query and provision cloud and SaaS resources using plain SQL.
- Providers become schemas; resources become tables: `github.repos.repos`, `aws.ec2.instances`, `google.compute.instances`.
- `SELECT` to read state. `INSERT` / `DELETE` / `EXEC` to provision and manage.
- No server or database to operate: provider definitions are derived from OpenAPI, and a SQLite-backed engine turns SQL into REST calls at query time.

Speaker notes: think "the cloud as a database you never have to load". The engine is stateless; it calls the live APIs under the hood and maps the responses into rows. Provider definitions are open and community-extensible.

### Slide a3: A query is just a query

```sql
SELECT name, default_branch, visibility
FROM github.repos.repos
WHERE org = 'stackql' AND archived = 0;
```

- One SQL surface over everything. The same shape whether it is GitHub repos or AWS security groups.
- Joins across providers work too: AWS resources against the GitHub repo that deploys them, for example.

Speaker notes: the point for today is that this is a uniform, structured, machine-friendly interface. Hold that thought, because it is exactly what an LLM wants.

## b) Intro to StackQL MCP

### Slide b1: MCP in one line

- Model Context Protocol: the standard way to give an LLM tools it can call. Think of it as the USB-C port for AI tooling.
- A model talks to an MCP server; the server exposes tools, the model calls them, results come back as structured content.

Speaker notes: keep this short, most of the room knows MCP. The interesting bit is what we put behind it.

### Slide b2: The StackQL MCP server

- Serves StackQL over MCP: `stackql mcp`.
- 13 tools, in three groups:
  - Discovery: `list_providers`, `list_services`, `list_resources`, `describe_resource`, `describe_method`, `list_methods`, `list_registry`, `server_info`
  - Query: `run_select_query`, `validate_select_query`
  - Change: `run_mutation_query`, `run_lifecycle_operation`, `pull_provider`
- Safety is a server-side contract, not a client promise: `read_only`, `safe`, `delete_safe`, `full_access`. Default is the most restrictive.

Speaker notes: the key idea: instead of writing N bespoke tools for N APIs, the agent gets one SQL-shaped tool surface over the entire cloud and SaaS estate. And read_only is enforced inside the server, so an agent literally cannot mutate when you start it read_only, no matter what the model decides.

## c) Agentic use cases for StackQL

### Slide c1: Why SQL and agents fit

- One tool surface for every provider: the model does not need a different tool per API.
- The model already speaks SQL fluently. No bespoke tool schema to teach.
- Results are tabular and structured: clean grounding, easy to cite, easy to verify.
- `read_only` is a real, enforceable safety boundary for letting an agent loose on production data.

Speaker notes: a lot of agent tool design is fighting the surface area. Here the surface area is one well-understood query language and the model is already good at it.

### Slide c2: Three pillars, one agent

- Agentic platform engineering: estate consistency and golden-path conformance. "Which repos drift from our standard: no branch protection, no CI, no required reviews?"
- Agentic SRE: live operational signal and triage. "What failed recently, on which branch, and what changed?"
- Agentic audit: IGA and entitlements (who can do what), CSPM (security posture), FinOps (cost). Point-in-time checks with evidence.

Speaker notes: these look like three different products. They are not. Watch the demo: it is one binary and one agent. The only thing that changes between a platform-engineering agent and an SRE agent is the system prompt. The backend, the tools, the safety mode are identical.

### Slide c3: The shape of an agentic answer

- Natural-language question in.
- Agent decides which StackQL queries to run, runs them via MCP, reads the rows.
- Answer out, grounded in the SQL it actually executed (which we show on screen).

Speaker notes: this is not text-to-SQL with one query. It is a loop: discover schema if needed, query, look at results, query again, then answer. The multi-turn tool loop is where "agent" earns the name.

## d) Embedded and vendored MCP: motivation and Rust primitives

This is the technical heart of the talk.

### Slide d1: The usual MCP topology, and why it is wrong for shipped software

- Normal MCP: the server is a separate process or service you deploy, and clients connect to it over a network.
- Great for shared, multi-tenant capability.
- Bad for distributable agent software: now your users install, secure, version, and operate two things instead of one. The server is a moving part you do not control on their machine.

Speaker notes: if you are building an agent that other people run, the deployed-server model pushes your problems onto them.

### Slide d2: Invert it: embed the server in your binary

- The agent process owns the server lifecycle: acquire the `stackql` binary, spawn it as a child over stdio, complete the MCP handshake, hand back a connected client.
- The server becomes an implementation detail of your app, not a dependency your users manage.
- Two acquisition modes behind one API:
  - Sidecar (default): download the platform `.mcpb` bundle at first run, verify its sha256 against pins baked into the crate, cache it. Offline after first run.
  - Vendored: `include_bytes!` the bundle into your binary and extract on first run. Zero network at runtime. One artifact to ship.

Speaker notes: sidecar is the small-download, always-current path. Vendored is the "hand someone a single file and it works on a plane" path. Same API, you pick at build time.

### Slide d3: The Rust crate

- `stackql-mcp` on crates.io. The dependency surface is deliberately tiny: rmcp, ureq, zip, sha2, serde.

```rust
use stackql_mcp::{Mode, StackqlMcp};

let server = StackqlMcp::builder()
    .mode(Mode::ReadOnly)
    .auth(serde_json::json!({ "github": { "type": "null_auth" } }))
    .start()       // acquires + spawns + MCP handshake
    .await?;       // -> RunningServer, derefs to a connected rmcp client
```

- Vendoring is a macro plus a build-time env var:

```rust
let server = StackqlMcp::builder()
    .bundle_bytes(stackql_mcp::include_bundle!())  // embedded at compile time
    .start().await?;
```

Speaker notes: `start()` is the whole acquisition state machine: resolve the platform, check the shared cache, download and verify if needed (or use the embedded bytes), extract, spawn, handshake. The caller sees one call and gets a connected client.

### Slide d4: Wiring it into an agent (rig + rmcp)

- The Rust agent ecosystem we stand on: `rig` (agent framework, 0.38) and `rmcp` (the official Rust MCP SDK, 1.7). `stackql-mcp` hands back an rmcp client, which is exactly what rig consumes.

```rust
// embedded StackQL server -> rig agent, in about ten lines
let server = StackqlMcp::builder().mode(Mode::ReadOnly).auth(auth)
    .bundle_bytes(stackql_mcp::include_bundle!()).start().await?;

let tools = server.list_all_tools().await?;     // the 13 StackQL tools
let sink  = server.peer().to_owned();            // the rmcp client peer

let agent = anthropic::Client::from_env()?
    .agent(anthropic::completion::CLAUDE_OPUS_4_8)
    .preamble(PERSONA_SYSTEM_PROMPT)             // <- the only thing that changes
    .rmcp_tools(tools, sink)                     // rig consumes rmcp directly
    .default_max_turns(20)
    .build();

let answer = agent.prompt("which repos lack branch protection?").await?;
```

Speaker notes: that is the entire integration. `list_all_tools()` and `peer()` come straight off our embedded client; rig's `rmcp_tools()` takes them as-is because we are both on rmcp 1.7. The persona is a string. Everything else is identical across the three agents.

### Slide d5: Why Rust for this

- One static binary, no runtime, fast cold start. The vendoring story (`include_bytes!`) is native to the language, not a hack.
- Memory safety matters for a thing that spawns child processes and parses untrusted zip and JSON.
- The ecosystem is here now: rig for agents, rmcp as the official MCP SDK. This is not a toy.

Speaker notes: the "single self-contained binary that is also an AI agent over your entire cloud estate" is a genuinely Rust-shaped artifact. You would fight the language to do this in most others.

## e) Demo

Run the live demo here. Full runbook with exact commands and fallbacks is in the appendix below. The arc:

1. Pre-flight: `stackql-agent --check`. Show it start the embedded server and list the 13 tools. No API key, no cloud creds.
2. Platform persona: ask about branch protection or missing licenses across the org. Watch it call `run_select_query` live (the tool calls print as it goes), then answer grounded in the rows.
3. SRE persona: same binary, `--persona sre`. Ask about recent failed workflow runs. Point out: different agent, identical backend, the system prompt is the only change.
4. Audit persona: `--persona audit`. Ask for unprotected default branches or public org members. Frame IGA/CSPM/FinOps on real credentials as the same code pointed at a credentialed provider.
5. The single-binary reveal: `ls -lh` the vendored release binary, then run it from a clean home directory with no network and no credentials. It still works, because the server is inside it.
6. Optional counterpoint: auditron, the deterministic sibling. When you need reproducible evidence rather than a conversation, the same embedded backend runs YAML control packs and emits an auditor-ready evidence zip.

Speaker notes: the emotional beat is step 3, the persona swap. Build to it. The technical beat is step 5, pulling the network and watching an 80-120 MB binary still answer questions about a live cloud estate.

## f) The ask

- It is all open source, MIT.
- `cargo add stackql-mcp`. Docs on docs.rs.
- Repos:
  - stackql/stackql-mcp-rs: the crate, plus the stackql-agent and auditron demos and the control packs
  - stackql/stackql: the engine
- Ways to get involved:
  - Star the repos if this is useful (it genuinely helps).
  - Try the agent against your own org or cloud account.
  - Contribute: control packs are community-extensible YAML, provider coverage is open, and new agent personas are a pull request against one file.
- One line to close on: a platform engineer, an SRE, and an auditor, in a single binary, that you can hand to someone on a plane.

Speaker notes: be specific about the ask. "Star the repo and try it against your own GitHub org tonight, it needs zero credentials" is a concrete call to action the room can act on.

## Appendix: demo runbook

Rehearse this end to end at least twice before the talk. The failure modes are network and rate limits, both of which have mitigations below.

### Pre-flight (do this before you walk on stage)

```sh
# 1. Build the vendored single binary once, ahead of time.
export ANTHROPIC_API_KEY=sk-ant-...           # your key
BUNDLE=$(cargo run -q -p stackql-mcp --example fetch_bundle)
STACKQL_MCP_BUNDLE_FILE=$BUNDLE cargo build -p stackql-agent --features vendored --release
AGENT=target/release/stackql-agent            # the artifact you will demo

# 2. Warm the provider cache and prove the machine is ready (no model calls).
$AGENT --check --persona platform

# 3. Warm the GitHub data path so the first real query is instant and to avoid
#    the unauthenticated rate limit biting mid-demo. A GitHub token lifts the
#    limit from 60/hour to 5000/hour - strongly recommended on conference wifi.
export GITHUB_TOKEN=ghp_...                    # optional but recommended
$AGENT --persona platform -p "list 3 repos in the stackql org with their visibility"
```

### The three persona runs (exact commands)

Platform engineering:

```sh
target/release/stackql-agent --persona platform
# then type:
#   Which repos in the stackql org are missing branch protection on their default branch?
#   Which stackql org repos have no license, and why is that a problem?
```

SRE:

```sh
target/release/stackql-agent --persona sre
# then type:
#   Show the most recent failed GitHub Actions workflow runs for stackql/stackql.
#   What's the pass/fail breakdown on the main branch?
```

Audit:

```sh
target/release/stackql-agent --persona audit
# then type:
#   Audit branch protection across the stackql org: which default branches are unprotected?
#   Who are the public members of the stackql org, and are any org owners?
```

One-shot mode (good for a scripted, deterministic demo where you do not want to type live):

```sh
target/release/stackql-agent --persona sre -p "Show the most recent failed workflow runs for stackql/stackql"
```

### What runs live vs what is the credentialed extension

Be honest about this on stage; it is a strength, not a weakness.

- Live, zero credentials (GitHub null_auth, public data):
  - Platform engineering: repo config, branch protection, default branches, licenses, staleness.
  - SRE: GitHub Actions workflow runs, conclusions, branches.
  - Audit posture: branch protection, public org membership, license compliance.
- Credentialed extension (same code, point it at a provider with auth):
  - IGA and entitlements over private org membership, teams, and repo collaborators.
  - CSPM over code scanning, secret scanning, and Dependabot alerts.
  - FinOps over billing and cost.
  - AWS, Google, Azure: same agent, `--auth` with the provider's auth document.

The line to say: "everything you are seeing needs no credentials at all, because it is public GitHub data. Point the exact same binary at a credentialed provider and you have IGA, CSPM, and FinOps over your real estate. The agent does not change."

### Fallback plan

- No or flaky wifi: the vendored binary needs no network to start (the server is embedded). It does need network to reach the GitHub API and the Anthropic API. If wifi is dead, fall back to a screen recording or asciinema of a rehearsed run. Record this in advance regardless.
- GitHub rate limit (HTTP 403): unauthenticated is 60/hour per IP. Set `GITHUB_TOKEN` before the talk to get 5000/hour. If you still hit it, switch to the one-shot commands you warmed during pre-flight (cached) or to auditron, which makes fewer calls.
- Anthropic API hiccup: switch to auditron, the deterministic sibling. It runs the same embedded backend with no LLM in the path and produces the evidence zip, so you still demo embedded MCP and cloud-as-SQL without depending on a model endpoint.
- A query comes back empty and surprising: that can mean a provider error surfaced as an empty result set, not a real "all clear". The audit persona is told about this; if it happens, say so and run a broader query. (This is also a known upstream issue we have filed: tool results should distinguish empty from errored.)

### Pre-flight checklist (tick before you start)

- [ ] Vendored release binary built and on PATH or at a known path.
- [ ] `ANTHROPIC_API_KEY` exported in the demo shell.
- [ ] `GITHUB_TOKEN` exported (lifts the rate limit).
- [ ] `stackql-agent --check` run once on the demo machine (warms the cache, proves it works).
- [ ] Each persona's first prompt run once during pre-flight (warms data, confirms answers).
- [ ] Screen recording of a full rehearsed run saved locally as the ultimate fallback.
- [ ] Font size up, terminal colors high-contrast, tool-call traces visible.
