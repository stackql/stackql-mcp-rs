# stackql-mcp launch kit

Working drafts for the crates.io launch of `stackql-mcp` and the auditron demo. Blog publishes first; the Reddit and This Week in Rust posts point back to it.

Placeholders to fill before posting:

- `BLOG_URL` - final URL of the blog post on stackql.io
- `ASCIINEMA_URL` - recording of the auditron TUI scan

## a) Blog post (Docusaurus)

Suggested path: `blog/2026-06-embedded-mcp-rust.md`

~~~markdown
---
title: An embedded StackQL MCP server for Rust agents
description: Query and provision cloud infrastructure over SQL from inside your Rust agent process, with no external server to deploy. Plus auditron, a compliance copilot in a single binary.
authors: [jeffreyaven]
tags: [mcp, rust, agents, compliance]
---

StackQL exposes cloud and SaaS providers (AWS, GitHub, Google, Azure, and
more) as SQL tables: `SELECT` to query your estate, `INSERT`/`DELETE` to
provision and tear down. The StackQL MCP server makes that capability
available to AI agents over the Model Context Protocol.

Today we are releasing [stackql-mcp](https://crates.io/crates/stackql-mcp),
a Rust crate that embeds that server inside your agent process. No
deployment, no sidecar to operate, no network dependency at runtime if you
choose so. It joins the npm and PyPI wrappers in the embedded MCP family,
sharing the same binary cache and the same launch contract.

<!-- truncate -->

## Why embedded

MCP servers are usually deployed as standalone processes that agents
connect to. That is the right shape for shared, multi-tenant capability,
but it is a poor fit for distributable agent software: your users now have
two things to install, version, and secure.

The embedded model inverts this. Your Rust binary owns the server
lifecycle: it acquires the `stackql` binary, launches it as a child over
stdio, completes the MCP handshake, and hands you a connected client from
[rmcp](https://crates.io/crates/rmcp), the official Rust MCP SDK. The
server is an implementation detail of your app.

```rust
use stackql_mcp::{Mode, StackqlMcp};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let server = StackqlMcp::builder()
        .mode(Mode::ReadOnly)
        .auth(serde_json::json!({"github": {"type": "null_auth"}}))
        .start()
        .await?;
    let tools = server.list_all_tools().await?;
    println!("{} tools available", tools.len());
    server.shutdown().await?;
    Ok(())
}
```

Two acquisition modes sit behind that one API:

- **Sidecar** (the default): the platform's `.mcpb` bundle is downloaded at
  first run, verified against sha256 pins baked into the crate at release
  time, and cached under `~/.stackql/mcp-server-bin/` - the same cache the
  npm and PyPI wrappers use. Subsequent starts are offline.
- **Vendored**: the bundle is embedded in your binary with
  `include_bytes!` and extracted on first run. No network at runtime, one
  artifact to ship.

```rust
let server = StackqlMcp::builder()
    .bundle_bytes(stackql_mcp::include_bundle!())
    .start()
    .await?;
```

Safety is a first-class part of the contract. The server enforces a mode
per session (`read_only`, `safe`, `delete_safe`, `full_access`) and the
crate defaults to the most restrictive; escalation is an explicit caller
opt-in.

## auditron: an auditor in a single binary

To show what the vendored mode is for, the repo ships
[auditron](https://github.com/stackql/stackql-mcp-rs/tree/main/auditron),
a terminal compliance copilot. The business problem: compliance engineers
run point-in-time control checks and need auditor-ready evidence, and the
usual answer involves cloud consoles and screenshots.

auditron runs a YAML control pack (id, description, SQL, pass criteria per
control) through the embedded server in read-only mode and streams
pass/fail/error into a ratatui table as results arrive. The SQL that
produced each finding is always on screen. Select a finding and press `e`
and Claude explains it and drafts remediation steps.

[ASCIINEMA_URL]

Then there is the part the auditor actually wants:

```sh
auditron evidence --out evidence-2026-06.zip
```

The zip contains the run manifest (pack checksum, collector identity,
server version, timings), the exact pack source and per-control SQL, and
per-control CSVs. Another auditor can re-run the same pack against the
same estate and diff the results.

The demo pack checks GitHub org security posture (branch protection,
default branches, licenses, staleness) using the github provider in
`null_auth` mode, so it runs against public org data with zero
credentials. Point it at your own org:

```sh
auditron scan --var org=your-org
```

Control packs are data, not code. They live in the repo as
`controls/*.yaml` and are community extensible.

## Lessons from building on live cloud APIs

Two things we hit while building the demo are worth knowing if you build
on this stack:

First, MCP tool results from the server carry the typed payload in
`structuredContent` and a markdown rendering in the text content. Read the
structured payload; parsing the markdown is a mistake we made so you do
not have to.

Second, provider API errors (rate limits especially) can surface as empty
result sets rather than tool errors. For a compliance tool, where "no rows"
means "compliant", that distinction is everything. The github pack includes
an inventory control that must return rows, doubling as a canary for the
whole run; we have also raised the underlying behavior upstream.

## Getting started

```sh
cargo add stackql-mcp
```

The crate is on [crates.io](https://crates.io/crates/stackql-mcp) with API
docs on [docs.rs](https://docs.rs/stackql-mcp). The source, the auditron
demo, and the control packs are at
[stackql/stackql-mcp-rs](https://github.com/stackql/stackql-mcp-rs), and
the engine itself lives at
[stackql/stackql](https://github.com/stackql/stackql). Issues and control
pack contributions welcome.
~~~

## b) Community posts

### Reddit (r/rust)

Title:

~~~text
stackql-mcp: embed a cloud-querying MCP server in your Rust agent (and auditron, a compliance TUI in a single 80MB binary)
~~~

Body:

~~~markdown
We just published [stackql-mcp](https://crates.io/crates/stackql-mcp), a
crate that embeds the StackQL MCP server (cloud infrastructure as SQL:
AWS, GitHub, Google, Azure...) inside a Rust process. Builder API in,
connected [rmcp](https://crates.io/crates/rmcp) client out:

    let server = StackqlMcp::builder()
        .mode(Mode::ReadOnly)
        .auth(json!({"github": {"type": "null_auth"}}))
        .start()
        .await?;

Things r/rust might find interesting:

- Two acquisition modes behind one API: sidecar (download at first run,
  sha256-verified against pins baked into the crate, shared cache) and
  vendored (`include_bytes!` the server bundle, extract on first run).
  The vendored mode is the fun one: your agent plus a full SQL engine for
  cloud APIs in one shippable binary, no network needed at runtime.
- The demo app, auditron, is a ratatui compliance copilot: it streams
  control checks (defined as YAML with SQL and pass criteria) into a live
  table, always shows the SQL behind a finding, has Claude draft
  remediation steps on demand, and exports an auditor-ready evidence zip
  (manifest, exact SQL, per-control CSVs - re-runnable by an auditor).
  The github demo pack runs unauthenticated against public org data, so
  `cargo run -p auditron -- scan` works with zero credentials.
- Dependency surface is deliberately small: rmcp, ureq, zip, sha2,
  serde. Server modes (read_only by default) are enforced server-side,
  not by the client promising to behave.

Blog with the design rationale: BLOG_URL
Repo (crate + demo + control packs): https://github.com/stackql/stackql-mcp-rs
The engine: https://github.com/stackql/stackql

Feedback welcome, especially on the builder API surface and the vendored
build flow (compile-time env var + `include_bundle!` macro - we went back
and forth on build.rs vs explicit pipeline and chose explicit).
~~~

### This Week in Rust

For the "Project/Tooling Updates" section (PR adding a line to the next
draft in [rust-lang/this-week-in-rust](https://github.com/rust-lang/this-week-in-rust)):

~~~markdown
* [stackql-mcp](https://github.com/stackql/stackql-mcp-rs) - a new crate
  embedding the StackQL MCP server (query AWS, GitHub, Google and more
  over SQL) in Rust agentic apps, with sha256-pinned sidecar download or
  a fully vendored single-binary mode; ships with
  [auditron](BLOG_URL), a ratatui compliance copilot demo that produces
  auditor-ready evidence packs
~~~

## c) Meetup talk synopsis

Title:

~~~text
An auditor in a single binary: embedded MCP backends for native Rust agents
~~~

Abstract (~150 words):

~~~text
AI agents are mostly wired together from hosted services: a model API
here, an MCP server deployed there. This talk makes the case for the
opposite shape: a native Rust agent that ships as one binary and owns its
entire backend.

We build up in three layers. First, the anatomy of a native Rust agent:
tokio, the official rmcp MCP SDK, and a child process speaking MCP over
stdio. Second, the embedded backend: stackql-mcp, a crate that embeds
StackQL (cloud infrastructure as SQL) with two acquisition modes -
sha256-pinned sidecar download, or a server bundle vendored into the
binary with include_bytes!, including what it takes to make extraction,
caching, and process lifecycle boring and reliable. Third, a real agent
on top: auditron, a terminal compliance copilot that runs SQL control
packs against live cloud APIs, explains findings with an LLM, and emits
re-runnable, auditor-ready evidence zips.

Live demo: zero credentials, one binary, real findings.
~~~

Talk notes (not for publication):

- Demo flow: `auditron scan` TUI against a public GitHub org (null_auth,
  no setup on conference wifi beyond GitHub API reachability; mind the
  60 req/h unauthenticated rate limit - warm the cache or bring a token)
- War stories section maps to the upstream issues: markdown vs
  structuredContent, boolean typing across resources, errors surfacing as
  empty result sets and the canary-control mitigation
- Closing slide: cargo add stackql-mcp, repo links, control packs as the
  community contribution surface
