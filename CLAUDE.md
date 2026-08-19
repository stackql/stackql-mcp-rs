# CLAUDE.md

## What this repo is

Demo code and runbook for the talk "Rust-native agentic platform engineering with embedded MCP", delivered by Jeff Aven at the Rust User Group Melbourne (late August 2026). Everything in here exists to support that talk. It is not a library and not a product; it is a working, repeatable demonstration that marries up with the slide deck.

The deck is mastered in Google Slides:
https://docs.google.com/presentation/d/15FpxQqUQ6WQdQYkHIL_CgVkKo3DgzZa_75VQYyzbEgo/edit

A PDF export of the deck lives in `ref/` (gitignored) and is the reference for what the demo needs to show. `slides/notes.md` holds paste-ready content for the slides still to be written. When the deck and this file disagree, the deck wins and this file gets updated.

## The talk in one paragraph

StackQL treats cloud and SaaS providers as data sources accessed via SQL. Agents in the platform engineering / SRE / observability space need to query, reason about, and act on actual running state, not on state files. The StackQL MCP server is the agent interface to the StackQL engine, with a small fixed tool surface (16 tools in v0.10.601) and safety modes gating writes. The `stackql-mcp` crate embeds that MCP server in a Rust application, either as a sidecar (resolved from the latest release, downloaded and verified at first run) or vendored straight into the Rust application (single self-contained binary). The concrete example is `steward`, a platform-engineering agent that keeps repositories on a golden path and repairs drift with a human approving each write.

## Demo flow

Three acts, in this order, matching the deck. Each act is runnable on its own from a clean shell in the repo root. Slot is 45 minutes including slides: roughly 40% on the primer side (act 1 + act 2 + the StackQL slides) and 60% on embedded MCP (the MCP and Rust slides + act 3).

Act 1 - StackQL primer (slides "STACKQL >>", "Cloud Providers as Data Sources", "How to use StackQL"), about 7 minutes of demo

- `primer/01-shell.sh` - `stackql shell`, discovery (`SHOW PROVIDERS`, `SHOW SERVICES IN github LIKE 'repo%'`, `SHOW RESOURCES IN github.repos LIKE '%branch%'`, `SHOW METHODS`, `DESCRIBE`), SELECTs and a JOIN pasted from `primer/queries/`.
- `primer/02-exec-formats.sh` - `stackql exec`, output formats `table | json | jsonl | csv` (those are the ones the engine has), `-H`, `-d`, piped into `column` and `jq`.
- `primer/03-srv-psql.sh` - `stackql srv` (Postgres wire protocol) and `psql`.

Provider: `github` with `null_auth` as the floor. `.env` creds lift the rate limit from 60/hour to 5000/hour; the queries are identical. `primer/queries/06-cross-provider.iql` (aws) is an optional flourish.

Act 2 - stackql-deploy, Rust native (slide "How to use StackQL", Platform Automation row), about 4 minutes

`stacks/golden-path/`: a repository's golden path as data (topics, standard labels, a default-branch ruleset, a conformance query). `stackql-deploy build|test|teardown stacks/golden-path dev --env-file .env`, `--dry-run --show-queries` first. Targets `GITHUB_ORG`/`GITHUB_REPO` from `.env` (defaults: this repo under stackql-labs). Needs a GitHub token in `.env` for `build`; `--dry-run` does not.

Act 3 - Embedded MCP in a Rust application (slides "STACKQL MCP" through "DEMO"), about 13 minutes

1. `embedded/minimal` - smallest program on the `stackql-mcp` crate: builder, `Mode::ReadOnly`, github `null_auth`, `list_all_tools`, one `run_select_query`, one refused `run_mutation_query`, shutdown. Sidecar acquisition; show the cache dir before and after.
2. `embedded/minimal-vendored` - same program with `bundle_bytes(include_bundle!())`; `build.rs` fetches the bundle at build time. Show the binary size and a start with a fresh `HOME`.
3. `embedded/steward` - the agent. `preflight` (server + tools, no model), `check` (read-only drift report against `policies/golden-path.md`), `scripts/drift.sh`, `check` again, `fix` (server in `Mode::Safe`, MCP elicitation answered at the terminal, `y`/`n` per write), `sql` (one statement, no model, the deterministic fallback), then the vendored release binary started on a fresh `HOME`.

Safety modes get called out during step 1 or 3: `ReadOnly` (default) -> `Safe` -> `DeleteSafe` -> `FullAccess`, escalation is a caller opt-in via `.mode(...)`; `Safe` asks the client for approval per write over MCP elicitation, and a client that cannot answer gets a refusal.

`auditron` and `stackql-agent` ARE part of this repo now (embedded/ workspace members, with `controls/` at the root) - the reference apps that shipped alongside the crate before it moved into stackql/stackql `packaging/mcpb/cargo`.

## Repo layout

```
rust-embedded-mcp-with-stackql/
  CLAUDE.md
  README.md                  short, points to the talk and the runbook
  RUNBOOK.md                 exact commands in demo order, copy/paste-able
  .env.example               every variable, all optional
  ref/                       (gitignored) PDF export of the deck
  slides/notes.md            paste-ready text + mermaid for slides still to be written
  primer/                    act 1: queries/*.iql, 01-shell.sh, 02-exec-formats.sh, 03-srv-psql.sh, _lib.sh
  stacks/                    act 2: golden-path/ (stackql_manifest.yml, resources/*.iql), README.md
  embedded/                  act 3
    Cargo.toml               workspace; [patch.crates-io] to the crate repo until 0.2 is published
    minimal/                 sidecar example
    minimal-vendored/        vendored feature, build.rs fetches the bundle
    steward/                 the agent (clap + rig + rmcp ClientHandler with elicitation)
    README.md                sidecar vs vendored, safety modes, steward usage
  scripts/
    check-env.sh             tools on PATH, creds present, rust version
    prewarm.sh               providers, bundle cache, all builds, one run of everything
    drift.sh                 introduce drift on the target repo (label + topic)
    _env.sh                  sourced helper: .env, stackql args
```

## Conventions

Writing (README, RUNBOOK, slide notes, comments):

- Australian spelling.
- Hyphens only. No em dashes, no `--` used as punctuation.
- `->` for arrows. Nothing that is not on a QWERTY keyboard.
- Matter-of-fact prose. No hyperbole, no marketing adjectives.
- Say "cloud as data sources accessed via SQL", not "cloud as SQL tables". Say "vendored straight into the Rust application", not "into the binary".
- No stacked markdown headings (a heading immediately followed by another heading).

Code:

- Rust 2021 edition, MSRV 1.88 (set by `rmcp` 1.x via `stackql-mcp`).
- `cargo fmt` and `cargo clippy --all-targets -- -D warnings` clean before commit (run in `embedded/`).
- Examples are deliberately small. `minimal` and `minimal-vendored` must stay under a screen. `steward` is three source files; if it grows past that it has stopped being a demo.
- Every runnable thing has a comment at the top saying which act and slide it belongs to.
- No credentials in the repo. `.env.example` documents variables; `.env` is gitignored.

Shell scripts:

- `set -euo pipefail`, POSIX-ish bash, tested on macOS and Linux (Jeff presents from macOS, darwin-universal bundle). They also run under Git Bash on Windows.
- Each script prints the command it is about to run so the room can read it. Echoes go to stderr so stdout stays pipeable.
- Pass `--approot "$HOME/.stackql"` to `stackql` explicitly (Windows builds default the approot to the cwd).

## Demo-day constraints

- Assume conference wifi is bad or absent. `scripts/prewarm.sh` must leave the machine able to start every act offline: providers pulled, the server bundle cached for sidecar, vendored binaries already built, cargo deps in the local registry cache. Reads against GitHub and calls to Anthropic still need the network; the runbook has fallbacks.
- github `null_auth` is the floor. Everything must work with zero credentials; anything needing credentials (act 2 `build`, `steward fix`, the aws flourish) is marked as such in the runbook.
- Terminal font size and colour scheme are set for a projector. Output in act 1 fits an 80x24 terminal without wrapping; pick queries and `SELECT` columns accordingly.
- Provider quirks that bite on stage: `ORDER BY` + `LIMIT` in one SELECT applies the limit first (wrap in a subquery); `IN (...)` on a key column fans out to one API call per value; booleans compare as `0`/`1`; a JOIN pushes key params from the other table but filter the fan-out explicitly.

## Working in this repo with Claude

- Before writing anything for act 3, read the crate docs at docs.rs/stackql-mcp (source: stackql/stackql `packaging/mcpb/cargo`); the published crate API is the source of truth, not memory. The crate version equals the stackql release it embeds; the workspace pins the minor (`stackql-mcp = "0.10"`).
- Before writing stack files for act 2, check `stackql-deploy --help` for the current argument order and flags in the Rust build. Env vars are referenced in manifests as `{{ GITHUB_ORG }}` (no `vars.` prefix). One statement per anchor.
- When asked for slide content, produce paste-ready text and mermaid source in `slides/notes.md`, not slide files. The deck lives in Google Slides.
- When asked for runbook changes, keep RUNBOOK.md strictly in demo order and copy/paste-able. No prose between commands beyond one line of intent.
- Table format by default when showing StackQL results in discussion.

## References

- stackql-mcp crate: https://crates.io/crates/stackql-mcp (source: https://github.com/stackql/stackql/tree/main/packaging/mcpb/cargo); auditron and stackql-agent live in this repo
- StackQL MCP docs: https://stackql.io/docs/mcp and https://stackql.io/docs/command-line-usage/mcp
- Installing (including the MCP server): https://stackql.io/docs/installing-stackql#installing-the-mcp-server
- stackql-deploy (Rust): https://github.com/stackql/stackql-deploy-rs, installer at get-stackql-deploy.io, docs at https://stackql-deploy.io
- Bundles: built by stackql/stackql packaging/mcpb, published per release on https://github.com/stackql/stackql/releases
- Provider registry: https://registry.stackql.app, github provider docs at https://github-provider.stackql.io
