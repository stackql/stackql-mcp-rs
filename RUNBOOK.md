# Runbook

Exact commands in demo order. Everything runs from the repo root in one shell with `.env` exported (`set -a; . ./.env; set +a`, which the scripts also do for themselves; `stackql-deploy` needs it exported because the stackql server it spawns reads provider credentials from the process environment). Lines starting with `#` are the one line of intent. Time budget: 45 minutes including slides; timings below are the demo portions.

Providers on stage: `aws` (compute, storage, Cost Explorer; `awscc` for the act 2 writes) and `cloudflare` (edge) in all three acts, `github` (code) in act 1. The GitHub golden-path stack (`stacks/golden-path`) is the zero-credential fallback and is not in the main flow.

## Before walking on stage

```sh
cd rust-embedded-mcp-with-stackql
set -a; . ./.env; set +a
# night before, on good wifi: everything below leaves the machine able to start every act offline
for p in aws awscc cloudflare github; do stackql exec "REGISTRY PULL $p"; done   # providers in ~/.stackql
(cd embedded && cargo build --release)                              # both agents; finops-agent-vendored's build.rs fetches the bundle
./embedded/target/release/sre-agent-sidecar --check                 # sidecar server downloaded, verified, cached
./embedded/target/release/finops-agent-vendored --check             # vendored server extracted from the binary
(cd primer/pgwire-lite-app && npm install)                          # the act 1 Node app's deps
python -c "import pystackql, pandas"                                # the act 1 Python app's deps (pip install pystackql pandas)
stackql-deploy build stacks/service-footprint dev --env-file .env    # estate converged (27 s from nothing, ~40 s to re-converge)
stackql-deploy build stacks/golden-path dev --env-file .env          # the GitHub variant converged too
stackql-deploy build stacks/aws-webserver dev --env-file .env        # native aws variant, second row for the act 1 cross-provider query (~25 s)
./embedded/target/release/sre-agent-sidecar                                 # one warm model call, expect 4 x PASS
export PATH="$PWD/embedded/target/release:$PATH"
clear
```

Checklist the rehearsal keeps catching:

- [ ] `stackql`, `stackql-deploy`, `psql`, `jq`, `node` and `python` on PATH; the block above ran clean
- [ ] `.env` exported in BOTH tabs, not just passed as `--env-file`
- [ ] GitHub token in `.env` is authorised for the `stackql` org (a fine-grained token scoped elsewhere reads fine but gets 403/404 on writes)
- [ ] `curl http://rust-demo.stackql.xyz/` returns the page (instance takes ~90 s after a fresh build to serve)
- [ ] recording of a full run saved locally (the ultimate fallback)

Terminal: 80x24 visible, large font, high contrast. Two tabs: `demo` (everything below) and `spare` (second shell, repo root, `.env` exported).

## Act 1 - StackQL primer (about 7 minutes)

Slides: STACKQL >>, Cloud Providers as Data Sources, How to use StackQL. `primer/` is paste blocks, not scripts (`shell.iql`, `exec.sh`, `srv.sh`): one block at a time, more blocks in each file than the flow below uses.

```sh
# 1. interactive discovery and queries
stackql shell
```

Paste in order from `primer/shell.iql` (every query is about a second of API time): 1.1 to 1.4, then 1.6 (window), 1.8 (edge), 1.11 (FinOps), 1.12 (cross-plane); 1.5, 1.7, 1.9 and 1.10 if there is time.

```sql
REGISTRY PULL aws;
REGISTRY PULL cloudflare;
REGISTRY PULL github;
SHOW PROVIDERS;
SHOW SERVICES IN aws LIKE 'ec%';
SHOW RESOURCES IN cloudflare.dns LIKE '%record%';
SHOW EXTENDED METHODS IN aws.ec2.security_group_rules;
DESCRIBE EXTENDED cloudflare.dns.zones_dns_records;
SELECT instance_id, instance_type, JSON_EXTRACT(state, '$.name') AS state, public_ip_address AS ip, strftime('%Y-%m-%d', launch_time) AS launched, ROUND(julianday('now') - julianday(launch_time)) AS age_days FROM aws.ec2.instances WHERE region = 'ap-southeast-2';
WITH live AS (SELECT instance_id, tags FROM aws.ec2.instances WHERE region = 'ap-southeast-2' AND JSON_EXTRACT(state, '$.name') = 'running') SELECT instance_id, JSON_EXTRACT(t.value, '$.key') AS tag, JSON_EXTRACT(t.value, '$.value') AS value FROM live, json_each(live.tags, '$.item') t;
SELECT volume_id, size, volume_type, state, SUM(size) OVER (PARTITION BY state) AS gib_in_state FROM aws.ec2.volumes WHERE region = 'ap-southeast-2';
SELECT r.type, COUNT(*) AS records, GROUP_CONCAT(r.name, ', ') AS names FROM cloudflare.zones.zones z JOIN cloudflare.dns.zones_dns_records r ON r.zone_id = z.id WHERE z.name = 'stackql.xyz' GROUP BY r.type;
WITH ce AS (SELECT results_by_time FROM aws.ce.cost_and_usages WHERE region = 'us-east-1' AND TimePeriod = '{"Start":"2026-08-01","End":"2026-09-01"}' AND Granularity = 'MONTHLY' AND Metrics = '["UnblendedCost"]' AND GroupBy = '[{"Type":"DIMENSION","Key":"SERVICE"}]') SELECT JSON_EXTRACT(g.value, '$.Keys[0]') AS service, ROUND(JSON_EXTRACT(g.value, '$.Metrics.UnblendedCost.Amount'), 2) AS usd FROM ce, json_each(ce.results_by_time, '$[0].Groups') g ORDER BY usd DESC;
SELECT 'aws' AS plane, instance_id AS name, JSON_EXTRACT(state, '$.name') AS state FROM aws.ec2.instances WHERE region = 'ap-southeast-2' UNION ALL SELECT 'cloudflare', r.name, r.type FROM cloudflare.zones.zones z JOIN cloudflare.dns.zones_dns_records r ON r.zone_id = z.id WHERE z.name = 'stackql.xyz';
```

Ctrl-D to leave the shell.

```sh
# 2. same engine, non-interactive: formats, files, query files, jsonnet vars, dry run
stackql exec --output json "SELECT name, status, JSON_EXTRACT(plan, '$.name') AS plan FROM cloudflare.zones.zones" | jq .
stackql exec --output csv -f records.psv -H -d="|" "SELECT r.type, r.name, r.content FROM cloudflare.zones.zones z JOIN cloudflare.dns.zones_dns_records r ON r.zone_id = z.id WHERE z.name = '$DEMO_DOMAIN'" && cat records.psv
stackql exec -i primer/queries/exposure.iql --output csv
stackql exec -i primer/queries/finops.iql --iqldata primer/queries/vars.jsonnet --var month=$(date +%Y-%m) --output csv
stackql exec -i primer/queries/finops.iql --iqldata primer/queries/vars.jsonnet --var month=$(date +%Y-%m) --dryrun --output text

# 3. same engine, Postgres wire protocol: psql, then a Node app on the same socket
nohup stackql srv --pgsrv.port 5466 > stackql-srv.log 2>&1 &
psql -h localhost -p 5466 -U stackql -d stackql -c "SELECT instance_id, instance_type, JSON_EXTRACT(state, '$.name') AS state, public_ip_address AS ip FROM aws.ec2.instances WHERE region = '$AWS_REGION'"
node primer/pgwire-lite-app/app.js
pkill -f "stackql srv"

# 4. same engine from Python: pystackql drives the binary, pandas does the aggregation
python primer/pystackql-app/app.py

# 5. the second plane, one query: the instance the edge points at (Cloudflare zone by name -> its A records -> EC2, one statement)
stackql exec -i primer/queries/cross-provider.iql --iqldata primer/queries/footprint-vars.jsonnet --var zone=$DEMO_DOMAIN --var region=$AWS_REGION
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

## Act 3 - embedded MCP in a Rust application (about 13 minutes)

Slides: STACKQL MCP through DEMO. Two agents on the same crate, one per way of embedding the server. Each has the same four parts, model, prompts, context and MCP tools, and the room should see each part on screen once. Both run the server in `read_only`; they investigate and report, they cannot change anything.

Step 1, the sidecar agent (slide SIDECAR DETAILED): Claude, server acquired at first run.

```sh
# the whole program: builder, prompts compiled in, rig agent over the MCP tools, the loop
cat embedded/sre-agent-sidecar/src/main.rs

# the prompts are data: persona and StackQL context, then the sweep
cat embedded/sre-agent-sidecar/prompts/task.md

# cache before (empty on a clean machine; filled by the pre-flight block here), then the preflight: server, providers, 16 tools, no model call
ls ~/.stackql/mcp-server-bin/
sre-agent-sidecar --check

# the sweep on a converged estate: four checks, expect 4 x PASS (60 to 80 s, ~16 tool calls); every tool call is a SQL statement on stderr
sre-agent-sidecar
```

Say: `Mode::ReadOnly` is one builder line and the server enforces it; the agent can only ever read. `Safe` and `DeleteSafe` put each write to a human over MCP elicitation, same builder, one line.

Introduce drift, in the `spare` tab or on stage (each is one StackQL statement, shown as it runs):

```sh
# strip the owner tag, open 22 to the world, delete the A record, add a dangling record
./stacks/service-footprint/drift.sh tag ssh edge dangling

# the sweep again (~90 s): three ATTENTIONs (exposure, edge with the dangling record, governance), each with the SQL that would fix it (not run: read_only)
sre-agent-sidecar

# put the estate back, deterministically, no model: the stack restores the rule set, the tags and the A record (~70 s)
stackql-deploy build stacks/service-footprint dev --env-file .env

# the dangling record is not in the stack; remove it with the statement the agent offered
stackql exec "DELETE FROM cloudflare.dns.records WHERE zone_id = '$CLOUDFLARE_ZONE_ID' AND dns_record_id = '$(stackql exec -o csv -H "SELECT id FROM cloudflare.dns.zones_dns_records WHERE zone_id = '$CLOUDFLARE_ZONE_ID' AND name = 'rust-demo-old.stackql.xyz' AND type = 'A'")'"
```

Step 2, the vendored agent (slides SIDECAR VS VENDORED, VENDORED DETAILED): GPT-5, server compiled into the binary.

```sh
# one builder line of difference, and a build script that fetches the bundle
diff embedded/sre-agent-sidecar/src/main.rs embedded/finops-agent-vendored/src/main.rs | grep '^[<>]' | head -30
cat embedded/finops-agent-vendored/build.rs

# sizes: app vs app-with-engine-inside
ls -lh embedded/target/release/sre-agent-sidecar embedded/target/release/finops-agent-vendored

# a clean machine: fresh HOME, the server still comes from inside the binary
# (the provider pull needs the network, the server acquisition does not)
HOME=$(mktemp -d) finops-agent-vendored --check

# the report: month-to-date spend by service from Cost Explorer, tagging gaps, waste (3 to 4 min, ~25 tool calls; start it, talk over it, or FINOPS_AGENT_MODEL=gpt-5-mini for a faster run)
finops-agent-vendored
```

Say: one file, the StackQL engine inside, no downloads; a different model behind the same tools, the loop is rig's, the prompts are markdown. Every tool call was a SQL statement you could read on stderr.

Free-form questions work on both if the room asks: `sre-agent-sidecar "Which security groups in ap-southeast-2 allow port 22 from anywhere?"`, `finops-agent-vendored "What did EC2 cost us last month, by usage type?"`.

## Close

Slide: STACKQL >> (thank you). The ask: `cargo add stackql-mcp`, star [stackql/stackql](https://github.com/stackql/stackql) and this repo, write a `task.md` for your own estate and send it as a PR.

## Fallbacks

- No wifi: every binary starts offline after the pre-flight block; acts 1 to 3 need AWS, Cloudflare, Anthropic and OpenAI reachable to answer. Switch to the recording and narrate.
- Anthropic slow or down: `sre-agent-sidecar --model claude-sonnet-5`, or run the sweep's checks deterministically with `stackql-deploy test stacks/service-footprint dev --env-file .env`. OpenAI slow or down: `finops-agent-vendored --model gpt-5-mini`, or the Cost Explorer query from `embedded/finops-agent-vendored/prompts/system.md` straight through `stackql exec`.
- Wrong state at the start of act 3: `stackql-deploy build stacks/service-footprint dev --env-file .env`.
- GitHub rate limit in act 1 (HTTP 403): `.env` has `STACKQL_GITHUB_USERNAME` / `STACKQL_GITHUB_PASSWORD`, make sure it is exported.
- Model wanders: `sre-agent-sidecar --max-turns 12`, or a narrower question as the argument.
- A terminated instance still shows its tags for up to an hour after a teardown; the task tells the agent to ignore terminated instances, and `drift.sh` only targets a running one.
- GitHub writes fail with 403/404 but reads work: the token is not authorised for the `stackql` org; use a classic PAT with `repo` scope or a fine-grained token granted on the org.
