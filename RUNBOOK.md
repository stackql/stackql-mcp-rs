# Runbook

Exact commands in demo order. Everything runs from the repo root in one shell with `.env` exported (`set -a; . ./.env; set +a`, which the scripts also do for themselves; `stackql-deploy` needs it exported because the stackql server it spawns reads provider credentials from the process environment). Lines starting with `#` are the one line of intent. Time budget: 45 minutes including slides; timings below are the demo portions.

Providers on stage: `aws` + `awscc` (compute) and `cloudflare` (edge) for acts 2 and 3, `github` with `null_auth` for act 1. The GitHub golden-path variant (`stacks/golden-path`, `steward --policy golden-path`) is the zero-credential fallback and is not in the main flow.

## Before walking on stage

```sh
cd rust-embedded-mcp-with-stackql
set -a; . ./.env; set +a
./scripts/check-env.sh
./scripts/prewarm.sh                                        # night before, on good wifi
stackql-deploy build stacks/service-footprint dev --env-file .env    # estate converged (27 s from nothing, ~40 s to re-converge)
stackql-deploy build stacks/golden-path dev --env-file .env          # the GitHub variant converged too
./embedded/target/release/steward check                             # one warm model call (~35 s when all PASS)
export PATH="$PWD/embedded/target/release:$PATH"
clear
```

Checklist the rehearsal keeps catching:

- [ ] `stackql-deploy` and `psql` on PATH (check-env.sh says so)
- [ ] `.env` exported in BOTH tabs, not just passed as `--env-file`
- [ ] GitHub token in `.env` is authorised for the `stackql` org (a fine-grained token scoped elsewhere reads fine but gets 403/404 on writes)
- [ ] `curl http://rust-demo.stackql.xyz/` returns the page (instance takes ~90 s after a fresh build to serve)
- [ ] recording of a full run saved locally (the ultimate fallback)

Terminal: 80x24 visible, large font, high contrast. Two tabs: `demo` (everything below) and `spare` (second shell, repo root, `.env` exported).

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

# 4. the second plane, one query: the instance the edge points at (AWS + Cloudflare in one statement)
stackql exec --approot ~/.stackql "$(grep -v '^--' primer/queries/06-cross-provider.iql | tr '\n' ' ')"
```

## Act 2 - stackql-deploy, Rust native (about 4 minutes)

Slide: How to use StackQL (Platform Automation row).

```sh
# Rust binary, no Python
stackql-deploy info

# a service's footprint as data: SG + instance in AWS, A record in Cloudflare
cat stacks/service-footprint/stackql_manifest.yml
cat stacks/service-footprint/resources/dns_record.iql

# what would run
stackql-deploy build stacks/service-footprint dev --env-file .env --dry-run --show-queries

# converge (already converged from pre-flight: every resource "in the desired state", footprint_ok=true)
stackql-deploy build stacks/service-footprint dev --env-file .env

# tests are the same checks
stackql-deploy test stacks/service-footprint dev --env-file .env

# it is a real web server behind a real DNS name
curl http://rust-demo.stackql.xyz/
```

Do not run `teardown` on stage (terminating and recreating the instance takes a minute and changes the IP); `build` from nothing takes about 30 s if you want to show it in the `spare` tab.

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

Say: `Mode::ReadOnly` is the safety story in one line, enforced by the server.

Step 2, sidecar vs vendored (slides SIDECAR VS VENDORED, VENDORED DETAILED):

```sh
# one line of difference
diff embedded/minimal/src/main.rs embedded/minimal-vendored/src/main.rs
cat embedded/minimal-vendored/build.rs

# sizes: app vs app-with-server-inside
ls -lh embedded/target/release/minimal embedded/target/release/minimal-vendored

# a clean machine: fresh HOME, the server still comes from inside the binary
HOME=$(mktemp -d) minimal-vendored
```

Step 3, steward (slide DEMO). Start from the agent and work backwards: what are we giving it agency over? Read everything (`read_only`); write what the policy names, one statement at a time, with a human approving (`safe`); never what the mode forbids. Two builds of the same source are on PATH order here: use the sidecar build (`embedded/target/release/steward`) for this step, the vendored one for the reveal.

```sh
# what the agent gets: the embedded server, three providers pulled, 16 tools, no model call yet
steward preflight

# the policy is data: five controls across AWS and Cloudflare, with the SQL shapes for each fix
cat embedded/steward/policies/service-footprint.md

# read-only: report against actual state (estate is converged, expect 5 x PASS, ~35 s)
steward check
```

Introduce drift, in the `spare` tab or on stage (each is one StackQL statement, shown as it runs):

```sh
# strip the owner tag, open 22 to the world, delete the A record, add a dangling record
./scripts/drift-footprint.sh tag ssh edge dangling
```

```sh
# read-only again: four DRIFTs with the SQL that would fix each; no write is possible in this mode
steward check

# safe mode: the server asks before every write; you approve at the terminal.
# Run it on the VENDORED binary: the single file is now doing the real work.
embedded/target/vendored/release/steward fix
```

At each `[approval] allow this write? [y/N]` read the SQL aloud and answer `y`. Four prompts: a tag patch (awscc UPDATE), a rule removal (awscc DELETE), the A record recreated with the IP the agent read from AWS (cloudflare INSERT), the dangling record removed (cloudflare DELETE). Decline one with `n` if there is time; the agent reports it as refused and moves on. 75 to 110 s end to end.

```sh
# the modes table, live: delete_safe lets creates and updates through, only deletes ask
steward --mode delete_safe sql --write "DELETE FROM cloudflare.dns.records WHERE zone_id = '$CLOUDFLARE_ZONE_ID' AND dns_record_id = 'not-a-real-id'"   # answer n

# confirm with the deterministic path (no model)
stackql-deploy test stacks/service-footprint dev --env-file .env
```

The single-binary reveal (the fix you just ran WAS this binary):

```sh
# same source, two builds: sidecar app vs app-with-engine-inside
ls -lh embedded/target/release/steward embedded/target/vendored/release/steward

# sidecar acquired the server into the shared cache on first run
ls ~/.stackql/mcp-server-bin/

# vendored: a fresh HOME has nothing, it still starts (server extracts from the binary;
# the provider pull needs the network, the server acquisition does not)
HOME=$(mktemp -d) embedded/target/vendored/release/steward preflight
```

Say: one file, the StackQL engine inside, no downloads, and it just changed two clouds with a human approving each statement. Every tool call was a SQL statement you could read on stderr.

Optional extras if the room wants more (both in `embedded/`, both on the same crate): `auditron scan --no-tui` runs the YAML control pack in `controls/` against the stackql org deterministically (no model); `stackql-agent --persona sre -p "..."` is the free-form rig agent (~30 s a question).

## Close

Slide: STACKQL >> (thank you). The ask: `cargo add stackql-mcp`, star [stackql/stackql](https://github.com/stackql/stackql) and this repo, write a policy for your own estate (`steward --policy my-policy.md --provider ...`; the GitHub one needs zero credentials), send it as a PR.

## Fallbacks

- No wifi: every binary starts offline after `prewarm.sh`; acts 1 to 3 need AWS, Cloudflare and Anthropic reachable to answer. Switch to the recording and narrate.
- Anthropic slow or down: `steward sql --write "<statement from the policy>"` shows the approval prompt with no model in the path; `stackql-deploy build stacks/service-footprint dev --env-file .env` re-converges everything in about 40 s.
- Wrong state at the start of act 3: the same `build` command.
- GitHub rate limit in act 1 (HTTP 403): `.env` has `STACKQL_GITHUB_USERNAME` / `STACKQL_GITHUB_PASSWORD`, make sure it is exported.
- Model wanders: `steward check --max-turns 12`, or a narrower `steward ask "..."`.
- A terminated instance still shows its tags for up to an hour after a teardown; the policy tells the agent to ignore terminated instances, and `drift-footprint.sh` only targets a running one.
- GitHub writes fail with 403/404 but reads work: the token is not authorised for the `stackql` org; use a classic PAT with `repo` scope or a fine-grained token granted on the org.
