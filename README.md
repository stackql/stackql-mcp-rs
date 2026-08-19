# Rust-native agentic platform engineering with embedded MCP

Demo code and runbook for the talk of the same name by Jeff Aven at the Rust User Group Melbourne, August 2026. Everything here is a working, repeatable demonstration that lines up with the deck.

The idea in one paragraph: [StackQL](https://stackql.io) treats cloud and SaaS providers as data sources accessed via SQL. Agents doing platform engineering, SRE and audit work need to query, reason about and act on actual running state, not on state files. The StackQL MCP server is the agent interface to that engine, with a small fixed tool surface and safety modes gating writes. The [`stackql-mcp`](https://crates.io/crates/stackql-mcp) crate embeds that server in a Rust application, either as a sidecar (downloaded and verified at first run) or vendored straight into the Rust application (one self-contained binary). The worked example is `steward`, an agent that keeps repositories on a golden path and repairs drift with a human approving each write.

## Three acts

| Act | Directory | What happens | Needs |
|---|---|---|---|
| 1. StackQL primer | [primer/](primer/) | `stackql shell`, `stackql exec` in every output format, `stackql srv` with `psql` | `stackql`; no credentials |
| 2. stackql-deploy, Rust native | [stacks/](stacks/) | declare a repo's golden path (topics, labels, ruleset) and `build` / `test` / `teardown` it | `stackql-deploy`; a GitHub token for `build` |
| 3. Embedded MCP in Rust | [embedded/](embedded/) | `minimal` (sidecar), `minimal-vendored`, then `steward check` / `fix` on the same repo | `cargo` 1.88+; `ANTHROPIC_API_KEY` for the agent |
| Reference apps | [embedded/](embedded/) | `auditron` (terminal compliance copilot over [controls/](controls/) packs) and `stackql-agent` (three-persona rig agent) - the apps that shipped alongside the crate | as act 3 |

[RUNBOOK.md](RUNBOOK.md) has the exact commands in demo order. [slides/notes.md](slides/notes.md) has the paste-ready text and mermaid for the slides still to be written in the deck.

## Quick start

```sh
git clone https://github.com/stackql/rust-embedded-mcp-with-stackql
cd rust-embedded-mcp-with-stackql
cp .env.example .env            # optional: fill in tokens; every act has a zero-credential path
./scripts/check-env.sh          # stackql, stackql-deploy, cargo, psql, jq on PATH?
./scripts/prewarm.sh            # pull providers, cache the server, build all binaries (do this on good wifi)
```

Then, from the repo root:

```sh
./primer/02-exec-formats.sh                                        # act 1
stackql-deploy build stacks/golden-path dev --env-file .env --dry-run --show-queries   # act 2
./embedded/target/release/minimal                                  # act 3, smallest embedding
./embedded/target/release/steward check                            # act 3, the agent (read-only)
./scripts/drift.sh && ./embedded/target/release/steward fix        # act 3, drift and repair with approval
./embedded/target/release/auditron scan --no-tui                   # reference app: control pack, zero credentials
./embedded/target/release/stackql-agent --check                    # reference app: rig agent preflight
./embedded/target/release/stackql-agent -p "which of our public repos have no license?"   # needs ANTHROPIC_API_KEY
```

## Why this shape

- One engine, several consumption patterns: shell, exec, Postgres wire protocol, declarative deploy, MCP. Act 1 and act 2 show the first four; act 3 is the fifth.
- The agent's tools are the StackQL MCP tools, so what the agent decides to do is always a readable SQL statement. The tool-call trace is the audit trail.
- Safety is a server-side contract. `Mode::ReadOnly` refuses writes however the model is prompted. `Mode::Safe` asks a human before each write, over MCP elicitation, and `steward` shows what that looks like in a terminal.
- Sidecar and vendored are the same builder with one line of difference. The vendored build of `steward` is a single file that starts with no network.

## References

- Crate and docs: [`stackql-mcp` on crates.io](https://crates.io/crates/stackql-mcp), [docs.rs/stackql-mcp](https://docs.rs/stackql-mcp); source lives in [stackql/stackql `packaging/mcpb/cargo`](https://github.com/stackql/stackql/tree/main/packaging/mcpb/cargo). The crate version equals the stackql release it embeds - pin the minor (`stackql-mcp = "0.10"`).
- StackQL MCP server: [stackql.io/docs/mcp](https://stackql.io/docs/mcp)
- stackql-deploy: [stackql-deploy.io](https://stackql-deploy.io), [stackql/stackql-deploy-rs](https://github.com/stackql/stackql-deploy-rs)
- Server bundles: built by [stackql/stackql `packaging/mcpb`](https://github.com/stackql/stackql/tree/main/packaging/mcpb), published on [stackql/stackql releases](https://github.com/stackql/stackql/releases)
- Provider registry: [registry.stackql.app](https://registry.stackql.app)

MIT licensed. Stars, issues and pull requests here and on [stackql/stackql](https://github.com/stackql/stackql) are the ask at the end of the talk.
