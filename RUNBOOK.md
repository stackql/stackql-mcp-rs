# Runbook

Exact commands in demo order. Everything runs from the repo root in one shell. Lines starting with `#` are the one line of intent. Time budget: 45 minutes including slides; the timings below are the demo portions.

## Before walking on stage

```sh
cd rust-embedded-mcp-with-stackql
set -a; . ./.env; set +a          # STACKQL_GITHUB_USERNAME/PASSWORD, ANTHROPIC_API_KEY, GITHUB_ORG/REPO
./scripts/check-env.sh
./scripts/prewarm.sh              # night before, on good wifi; leaves everything cached and built
stackql-deploy build stacks/golden-path dev --env-file .env     # repo on the golden path before act 3
./embedded/target/release/steward check                         # one warm model call so the first on stage is not the first of the day
export PATH="$PWD/embedded/target/release:$PATH"
clear
```

Terminal: 80x24 visible, large font, high contrast. Two tabs: `demo` (everything below) and `spare` (a second shell already in the repo root, .env sourced).

## Act 1 - StackQL primer (about 7 minutes)

Slides: STACKQL >>, Cloud Providers as Data Sources, How to use StackQL.

```sh
# 1. interactive discovery
./primer/01-shell.sh
```

Paste in order (from `primer/queries/01-discovery.iql`, then 02, 03, 04):

```sql
SHOW PROVIDERS;
SHOW SERVICES IN github LIKE 'repo%';
SHOW RESOURCES IN github.repos LIKE '%branch%';
SHOW METHODS IN github.repos.branches;
SELECT visibility, archived, COUNT(*) AS repos FROM github.repos.repos WHERE org = 'stackql' GROUP BY visibility, archived;
SELECT name, stars, forks FROM (SELECT name, stargazers_count AS stars, forks_count AS forks FROM github.repos.repos WHERE org = 'stackql' AND archived = 0 ORDER BY stargazers_count DESC) t LIMIT 5;
SELECT b.repo, b.name AS branch, b.protected FROM github.repos.branches b JOIN github.repos.repos r ON r.name = b.repo WHERE r.org = 'stackql' AND b.owner = 'stackql' AND b.repo IN ('stackql', 'stackql-deploy', 'pystackql') AND b.name = r.default_branch;
```

Ctrl-D to leave the shell.

```sh
# 2. same engine, non-interactive, every output format, pipes
./primer/02-exec-formats.sh

# 3. same engine, Postgres wire protocol, any pg client
./primer/03-srv-psql.sh
```

Optional flourish if AWS creds are in `.env` (skip otherwise):

```sh
stackql exec --approot ~/.stackql "$(grep -v '^--' primer/queries/06-cross-provider.iql | tr '\n' ' ')"
```

## Act 2 - stackql-deploy, Rust native (about 4 minutes)

Slide: How to use StackQL (Platform Automation row).

```sh
# Rust binary, no Python
stackql-deploy info

# the golden path as data
cat stacks/golden-path/stackql_manifest.yml
cat stacks/golden-path/resources/label.iql

# what would run
stackql-deploy build stacks/golden-path dev --env-file .env --dry-run --show-queries

# converge (already converged from pre-flight: every resource reports "in the desired state")
stackql-deploy build stacks/golden-path dev --env-file .env

# tests are the same checks
stackql-deploy test stacks/golden-path dev --env-file .env
```

Do not run `teardown` on stage unless there is time; if you do, run `build` again straight after so act 3 starts from a converged repo.

## Act 3 - embedded MCP in a Rust application (about 15 minutes)

Slides: STACKQL MCP through DEMO.

Step 1, the smallest embedding (slide SIDECAR DETAILED):

```sh
# the whole program
cat embedded/minimal/src/main.rs

# cache before (empty on a clean machine; prewarmed here)
ls ~/.stackql/mcp-server-bin/

# run: 16 tools, one SELECT, one refused write
minimal

# cache after
ls ~/.stackql/mcp-server-bin/*/
```

Say: `Mode::ReadOnly` is the safety story in one line, enforced by the server. To show a real first-run download, run `spare` tab: `STACKQL_MCP_VERSION=0.10.591 minimal` (a release not yet cached) while talking.

Step 2, sidecar vs vendored (slides SIDECAR VS VENDORED, VENDORED DETAILED):

```sh
# one line of difference
diff embedded/minimal/src/main.rs embedded/minimal-vendored/src/main.rs
cat embedded/minimal-vendored/build.rs

# sizes: app vs app-with-server-inside
ls -lh embedded/target/release/minimal embedded/target/release/minimal-vendored

# a clean machine: fresh HOME, the server still comes from inside the binary
HOME=$(mktemp -d) minimal-vendored     # server extracts and starts; the query then wants the github provider pulled
```

Say: sidecar resolves the latest server release and verifies sha256; vendored ships the bytes. Same builder either side.

Step 3, steward (slide DEMO):

```sh
# what the agent gets: the embedded server, its tools, no model call yet
steward preflight

# the policy is data
cat embedded/steward/policies/golden-path.md

# read-only: report drift against actual state (repo is converged, expect PASS on 1-3, hygiene notes on 4)
steward check
```

Introduce drift, in the `spare` tab or on stage:

```sh
./scripts/drift.sh
```

```sh
# read-only again: DRIFT on topics and labels, proposed SQL, no writes possible
steward check

# safe mode: the server asks before each write, you approve at the terminal
steward fix
```

At each `[approval] allow this write? [y/N]` read the SQL aloud, answer `y`. Decline one (`n`) if there is time, to show the agent report a refused fix. Then:

```sh
# confirm with the deterministic path (no model)
steward sql "SELECT name, color FROM github.issues.labels WHERE owner='$GITHUB_ORG' AND repo='$GITHUB_REPO' AND name = 'security'"
stackql-deploy test stacks/golden-path dev --env-file .env
```

The single-binary reveal:

```sh
ls -lh embedded/target/vendored/release/steward
HOME=$(mktemp -d) embedded/target/vendored/release/steward preflight    # clean machine: server from inside the binary
```

With wifi off and the real HOME (providers already pulled by prewarm), `steward preflight` and `steward sql "SELECT ..."` still start; only the calls out to GitHub and Anthropic need the network.

Say: one file, the StackQL engine inside, no downloads, and it just answered questions about a live estate. Then the safety modes: ReadOnly -> Safe -> DeleteSafe -> FullAccess, opt-in via `.mode(...)`, and every tool call is a SQL statement you can read on stderr.

## Close

Slide: STACKQL >> (thank you). The ask: `cargo add stackql-mcp`, star [stackql/stackql](https://github.com/stackql/stackql) and this repo, run `steward check` against your own org tonight (it needs zero credentials for reads), and send a policy or a provider example as a pull request.

## Fallbacks

- No wifi: every binary starts offline after `prewarm.sh`, but act 1 and act 3 need GitHub (and act 3 Anthropic) reachable to answer anything. Switch to the recorded run (link in `slides/notes.md`) and narrate over it.
- GitHub rate limit (HTTP 403 in a result): you are on `null_auth` at 60/hour. Source `.env` with `STACKQL_GITHUB_USERNAME` / `STACKQL_GITHUB_PASSWORD` for 5000/hour.
- Anthropic error or slow: `steward sql --write "INSERT INTO github.issues.labels (owner, repo, name, color, description) SELECT '$GITHUB_ORG', '$GITHUB_REPO', 'security', 'b60205', 'Security posture or vulnerability'"` shows the same approval prompt with no model in the path; `stackql-deploy build` re-converges the repo.
- Wrong state at the start of act 3: `stackql-deploy build stacks/golden-path dev --env-file .env` puts it back.
- Model wanders: `steward check --max-turns 12` or ask a narrower question with `steward ask "..."`.
