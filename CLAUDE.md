# CLAUDE.md

## What this repo is

The long-lived Rust demo repo for embedded StackQL MCP: working, repeatable demonstrations of building agentic applications in Rust on the published `stackql-mcp` crate. It began as (and still contains) the demo code and runbook for the talk "Rust-native agentic platform engineering with embedded MCP" (Jeff Aven, Rust User Group Melbourne, August 2026), and additionally hosts the reference apps that shipped alongside the crate before the crate moved into stackql/stackql `packaging/mcpb/cargo`. It is not a library and not a product.

History note: this repository was previously `stackql/stackql-mcp-rs` (the crate's original home; renamed, stars and history preserved). The pre-repurpose crate tree is in git history before commit 464fab9.

The deck is mastered in Google Slides:
https://docs.google.com/presentation/d/15FpxQqUQ6WQdQYkHIL_CgVkKo3DgzZa_75VQYyzbEgo/edit

A PDF export of the deck lives in `ref/` (gitignored) and is the reference for what the demo needs to show. `slides/notes.md` holds the speaker notes, one block per slide; `slides/build_slides.py` regenerates slides 13-15 from a pptx export. When the deck and this file disagree, the deck wins and this file gets updated.

## The talk in one paragraph

StackQL treats cloud and SaaS providers as data sources accessed via SQL. Agents in the platform engineering / SRE / observability space need to query, reason about, and act on actual running state, not on state files. The StackQL MCP server is the agent interface to the StackQL engine, with a small fixed tool surface (16 tools) and safety modes gating writes. The `stackql-mcp` crate embeds that MCP server in a Rust application, either as a sidecar (the release pinned in the crate, downloaded and sha256-verified at first run) or vendored straight into the Rust application (single self-contained binary). The concrete example is `steward`, a platform-engineering agent that keeps a service's footprint across AWS and Cloudflare on its golden path and repairs drift with a human approving each write.

## Demo flow

Three acts, in this order, matching the deck. Each act is runnable on its own from a clean shell in the repo root. Slot is 45 minutes including slides: roughly 40% on the primer side (act 1 + act 2 + the StackQL slides) and 60% on embedded MCP (the MCP and Rust slides + act 3).

Act 1 - StackQL primer (slides "STACKQL >>", "Cloud Providers as Data Sources", "How to use StackQL"), about 7 minutes of demo

- `primer/01-shell.sh` - `stackql shell`, discovery (`SHOW PROVIDERS`, `SHOW SERVICES IN github LIKE 'repo%'`, `SHOW RESOURCES IN github.repos LIKE '%branch%'`, `SHOW METHODS`, `DESCRIBE`), SELECTs and a JOIN pasted from `primer/queries/`.
- `primer/02-exec-formats.sh` - `stackql exec`, output formats `table | json | jsonl | csv` (those are the ones the engine has), `-H`, `-d`, piped into `column` and `jq`.
- `primer/03-srv-psql.sh` - `stackql srv` (Postgres wire protocol) and `psql`.

Provider: `github` with `null_auth` as the floor. `.env` creds lift the rate limit from 60/hour to 5000/hour; the queries are identical. `primer/queries/06-cross-provider.iql` joins Cloudflare DNS records to the EC2 instances they point at (needs the act 2/3 credentials) and is the bridge into the next two acts.

Act 2 - stackql-deploy, Rust native (slide "How to use StackQL", Platform Automation row), about 4 minutes

`stacks/service-footprint/`: one service's footprint across two planes as data: a security group and a tagged `t3.micro` in AWS (`awscc` writes, `aws` reads), an A record `rust-demo.stackql.xyz` in Cloudflare pointing at it, a conformance query. `stackql-deploy build|test|teardown stacks/service-footprint dev --env-file .env`, `--dry-run --show-queries` first. `.env` must be exported as well as passed (the spawned stackql server reads provider creds from the environment). Clean build about 30 s, re-converge about 40 s. `stacks/golden-path/` (GitHub: topics, labels, ruleset) is the zero-credential variant, not in the main flow.

Act 3 - Embedded MCP in a Rust application (slides "STACKQL MCP" through "DEMO"), about 13 minutes

1. `embedded/minimal` - smallest program on the `stackql-mcp` crate: builder, `Mode::ReadOnly`, github `null_auth`, `list_all_tools`, one `run_select_query`, one refused `run_mutation_query`, shutdown. Sidecar acquisition; show the cache dir before and after.
2. `embedded/minimal-vendored` - same program with `bundle_bytes(include_bundle!())`; `build.rs` fetches the bundle at build time. Show the binary size and a start with a fresh `HOME`.
3. `embedded/steward` - the agent. Start from the agent and work backwards (what does it have agency over: read everything, write what the policy names with a human approving each statement, never what the mode forbids). `preflight` (server + three providers + tools, no model), `check` (read-only, five controls across AWS and Cloudflare, about 55 s), `scripts/drift-footprint.sh tag ssh edge dangling`, `check` again, `fix` (server in `Mode::Safe`, MCP elicitation answered at the terminal, four approvals: awscc tag patch, awscc rule delete, cloudflare record recreated from the IP read in AWS, cloudflare dangling record delete; about 110 s), `--mode delete_safe` to show only deletes asking, `sql` (one statement, no model, the deterministic fallback), then the vendored release binary started on a fresh `HOME`. Policies are markdown with `{{ ENV }}` placeholders, compiled in: `service-footprint` (default) and `golden-path` (GitHub, zero-credential reads); `--policy path` for your own.

Safety modes get called out during step 1 or 3: `ReadOnly` (default) -> `Safe` -> `DeleteSafe` -> `FullAccess`, escalation is a caller opt-in via `.mode(...)`; `Safe` asks the client for approval before every write (deletes included), `DeleteSafe` only for deletes, over MCP elicitation; a client that cannot answer gets a refusal.

`auditron` and `stackql-agent` ARE part of this repo now (embedded/ workspace members, with `controls/` at the root) - the reference apps that shipped alongside the crate before it moved into stackql/stackql `packaging/mcpb/cargo`.

## Repo layout

```
rust-embedded-mcp-with-stackql/
  CLAUDE.md
  README.md                  short, points to the talk and the runbook
  RUNBOOK.md                 exact commands in demo order, copy/paste-able
  .env.example               every variable, all optional
  ref/                       (gitignored) PDF export of the deck
  slides/notes.md            speaker notes per slide; build_slides.py regenerates slides 13-15
  primer/                    act 1: queries/*.iql, 01-shell.sh, 02-exec-formats.sh, 03-srv-psql.sh, _lib.sh
  stacks/                    act 2: service-footprint/ (aws + awscc + cloudflare), golden-path/ (github), README.md
  embedded/                  act 3
    Cargo.toml               workspace; stackql-mcp = "0.10" from crates.io (version-locked
                             to the stackql release it embeds; a server bump is a normal
                             dependency bump - never reintroduce a git pin)
    minimal/                 sidecar example
    minimal-vendored/        vendored feature, build.rs fetches the bundle
    steward/                 the agent (clap + rig + rmcp ClientHandler with elicitation); policies/*.md
    auditron/                reference app: terminal compliance copilot over controls/ packs
    stackql-agent/           reference app: rig agent over Claude; --check, -p one-shot, REPL
    README.md                sidecar vs vendored, safety modes, steward usage
  controls/                  YAML control packs for auditron (github-core runs with zero creds)
  .github/workflows/ci.yml   fmt, clippy -D warnings, build, zero-credential smokes, agent-live
  scripts/
    check-env.sh             tools on PATH, creds present, rust version
    prewarm.sh               providers, bundle cache, all builds, one run of everything
    drift-footprint.sh       introduce drift on the service (tag, ssh, edge, dangling), one stackql statement each
    drift.sh                 the GitHub variant (label + topic)
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
- The main flow needs AWS, Cloudflare and Anthropic credentials in `.env` (exported). The GitHub variant (`stacks/golden-path`, `steward --policy golden-path`, act 1) keeps a zero-credential path for people who clone the repo; the runbook marks what needs what.
- Terminal font size and colour scheme are set for a projector. Output in act 1 fits an 80x24 terminal without wrapping; pick queries and `SELECT` columns accordingly.
- Provider quirks that bite on stage: `ORDER BY` + `LIMIT` in one SELECT applies the limit first (wrap in a subquery); `IN (...)` on a key column fans out to one API call per value; booleans compare as `0`/`1`; a JOIN pushes key params from the other table but filter the fan-out explicitly; `UPDATE ... SET` sends numbers and booleans as strings (Cloudflare `ttl`, classic branch protection), so prefer `awscc` PatchDocument writes and delete+insert where a provider insists on types; a terminated EC2 instance keeps its tags readable for up to an hour.
- The demo mutates a real sandbox: AWS account 824532806693 (ap-southeast-2, default VPC) and the `stackql.xyz` zone. Identity tags (`service`, `stackql:*`) find the resources; governance tags (`owner`, `cost-centre`) are desired state. Never make an `exists` query depend on a tag that is allowed to drift.

## CI

`.github/workflows/ci.yml`, on every push to main and PR. `build-test`: fmt, clippy `-D warnings`, workspace build (the vendored build.rs downloads and pin-verifies the bundle - network needed), then zero-credential smokes: `minimal`, `auditron scan --no-tui`, `stackql-agent --check`. Convention that matters: auditron is a compliance gate whose exit 2 means "controls failed" - CI treats that as a successful run (`|| [ $? -eq 2 ]`); only a crash fails the job. `agent-live` runs one real one-shot `stackql-agent` prompt; the `ANTHROPIC_API_KEY` org secret is already configured, so pushes make one real (small) Claude call.

## Working in this repo with Claude

- Before writing anything for act 3, read the crate docs at docs.rs/stackql-mcp (source: stackql/stackql `packaging/mcpb/cargo`); the published crate API is the source of truth, not memory. The crate version equals the stackql release it embeds; the workspace pins the minor (`stackql-mcp = "0.10"`).
- The models default to Claude (`claude-opus-5` for steward via `STEWARD_MODEL`, `claude-sonnet-5` for stackql-agent). anthropic-sdk / rig read `ANTHROPIC_API_KEY` from the environment.
- auditron embeds `controls/github-core.yaml` via `include_str!("../../../controls/...")` - the path is relative to `embedded/auditron/src/`, and the cwd-based pack listing expects to run from the repo root.
- Before writing stack files for act 2, check `stackql-deploy --help` for the current argument order and flags in the Rust build. Env vars are referenced in manifests as `{{ AWS_REGION }}` (no `vars.` prefix). One statement per anchor. Read with `aws`, write with `awscc`; `cloudflare.dns.zones_dns_records` to list/create, `cloudflare.dns.records` to edit/delete.
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
