# Act 3 - embedded MCP in a Rust application

Three programs, one crate. All three embed the StackQL MCP server via [`stackql-mcp`](https://crates.io/crates/stackql-mcp) and talk to it through the [`rmcp`](https://crates.io/crates/rmcp) client it hands back.

| Program | Slide | What it shows |
|---|---|---|
| `minimal/` | SIDECAR DETAILED | The smallest embedding: builder, `Mode::ReadOnly`, github `null_auth`, list tools, one `run_select_query`, one refused `run_mutation_query`, shutdown. First run downloads and verifies the server; later runs are offline. |
| `minimal-vendored/` | VENDORED DETAILED | The same program with the server bundle compiled in. `build.rs` fetches the bundle once at build time; `include_bundle!()` embeds it; the crate extracts it on first run. No network at run time. |
| `steward/` | DEMO | The agent. A platform-engineering steward that reads actual repository state through the embedded server, compares it with a policy, reports drift, and (in `fix` mode) remediates it with every write approved by a human at the terminal. |
| `auditron/` | - | Reference app: a terminal compliance copilot. Runs the YAML control packs in [../controls/](../controls/) against live provider state (github pack needs zero credentials), renders a live TUI or `--no-tui` line output, and emits auditor-ready evidence packs. |
| `stackql-agent/` | - | Reference app: the crate wired into a [rig](https://docs.rs/rig-core) agent talking to Claude. One binary, three personas (`--persona platform | sre | audit`); `--check` preflights without a model call, `-p/--prompt` is one-shot, otherwise REPL. Needs `ANTHROPIC_API_KEY`. |

## Build

```sh
cd embedded
cargo build --release -p minimal -p steward             # sidecar
cargo build --release -p minimal-vendored               # vendored
cargo build --release -p steward --features vendored --target-dir target/vendored
```

MSRV 1.88. `stackql-mcp` comes from crates.io, pinned to the minor (`"0.10"`): the crate version equals the stackql release it embeds, so a server bump is a normal dependency bump.

## Sidecar vs vendored

Same builder, same connected client, one line of difference (`.bundle_bytes(stackql_mcp::include_bundle!())`) and one Cargo feature.

| | Sidecar (default feature) | Vendored (`vendored` feature) |
|---|---|---|
| How the server arrives | the release pinned in the crate, `.mcpb` downloaded and cached on first run | fetched once at build time (`build.rs` or `cargo run -p stackql-mcp --example fetch_bundle`), embedded with `include_bundle!()` |
| Integrity | sha256 checked against the pins rendered into the crate from the release's `.sha256` assets | the bytes are the artefact (fetched pin-verified at build time) |
| First run | download, verify, extract to `~/.stackql/mcp-server-bin/<version>/<platform>/`, launch | extract to `~/.stackql/mcp-server-bin/vendored/<hash>/`, launch |
| Network at run time | first run of each release only | never |
| Artefact size | small (the app) | app + ~40 MB (linux/windows) or ~90 MB (darwin-universal) |
| Choose when | you want small binaries and easy server updates | air-gapped hosts, single-file distribution, demos on conference wifi |

Overrides that work in both: `STACKQL_MCP_BIN` (run a stackql you already have), `STACKQL_MCP_BUNDLE` (extract a local `.mcpb`), `STACKQL_MCP_VERSION=latest|pinned|<version>`, `Builder::approot()`.

## Safety modes

`Mode::ReadOnly` (default) -> `Mode::Safe` -> `Mode::DeleteSafe` -> `Mode::FullAccess`. The mode is enforced inside the server: in `ReadOnly` a `run_mutation_query` is refused whatever the model asks; in `Safe` and `DeleteSafe` the server sends an MCP elicitation request before each gated write and refuses if the client cannot answer it. Escalation is a caller opt-in via `.mode(...)`.

`steward` uses this directly. `check` runs the server read-only. `fix` runs it in `Safe` mode and serves it with an rmcp `ClientHandler` that advertises elicitation and answers it by asking the operator (`embedded/steward/src/embed.rs`). The crate's `start()` uses a client that declines elicitation, so `steward` takes the escape hatch: `Builder::command()` gives the acquired binary and canonical launch arguments, and `steward` spawns and serves it itself.

## steward

```sh
export ANTHROPIC_API_KEY=sk-ant-...            # for check / fix / ask / chat
export GITHUB_ORG=stackql-labs GITHUB_REPO=rust-embedded-mcp-with-stackql

steward preflight                              # server + tools, no model calls
steward check                                  # read-only drift report with proposed SQL
steward fix                                    # remediate; approve each write at the terminal
steward fix --yes                              # scripted: auto-approve, decisions still logged
steward ask "Which repos in stackql-labs have no licence?"
steward sql "SELECT name FROM github.issues.labels WHERE owner='stackql-labs' AND repo='rust-embedded-mcp-with-stackql'"
steward sql --write "UPDATE ..."               # one write through the approval prompt, no model
```

Options: `--repo owner/name`, `--policy path` (default: the golden-path policy compiled in from `policies/golden-path.md`), `--provider`, `--auth <json>`, `--model` (default `claude-opus-5`, or `STEWARD_MODEL`), `--max-turns`. With `STACKQL_GITHUB_USERNAME` / `STACKQL_GITHUB_PASSWORD` set the server authenticates as that user (needed for `fix`); otherwise it runs `null_auth` against public data.

The policy is markdown. Write your own for your org's golden path; the agent reads it as instructions. Providers other than github work the same way: `--provider aws --auth '{"aws":{"type":"aws_signing_v4"}}'`.

The wiring, in full:

```rust
let server = embed::start(StackqlMcp::builder().mode(mode).auth(auth), ApprovingClient::new(yes)).await?;
let agent = anthropic::Client::from_env()?
    .agent("claude-opus-5")
    .preamble(&prompt::system_prompt(&policy, &repo, can_write))
    .rmcp_tools(server.list_all_tools().await?, server.peer().clone())
    .default_max_turns(24)
    .build();
```

Everything the agent does is a StackQL statement, so the tool-call trace on stderr is a readable audit trail of what it looked at and what it changed.
