# =============================================================================
# stackql exec - one-shot queries with output options
# copy and paste one block at a time (not a batch script)
# Act 1, slide "How to use StackQL" (Interactive Analytics row)
# prereqs: none for the github blocks (STACKQL_GITHUB_USERNAME and
# STACKQL_GITHUB_PASSWORD in .env lift the rate limit); the last block needs
# AWS_* and CLOUDFLARE_* exported from .env (set -a; . ./.env; set +a)
# Windows builds default --approot to the cwd: add --approot ~/.stackql there
# =============================================================================

# default output (table) to stdout
stackql exec \
"SELECT visibility, archived, COUNT(*) AS repos FROM github.repos.repos WHERE org = 'stackql' GROUP BY visibility, archived"

# json to stdout (pipe into jq)
stackql exec --output json \
"SELECT name, stars, forks FROM (SELECT name, stargazers_count AS stars, forks_count AS forks FROM github.repos.repos WHERE org = 'stackql' AND archived = 0 ORDER BY stargazers_count DESC) t LIMIT 5" | jq .

# jsonl (one object per line) written to a file
stackql exec --output jsonl -f repos.jsonl \
"SELECT name, language, stargazers_count, forks_count, archived FROM github.repos.repos WHERE org = 'stackql'"
head -5 repos.jsonl

# csv with headers to a file
stackql exec --output csv -f runs.csv \
"SELECT name, head_branch, conclusion, created_at FROM (SELECT name, head_branch, conclusion, created_at FROM github.actions.workflow_runs WHERE owner = 'stackql' AND repo = 'stackql' ORDER BY created_at DESC) t LIMIT 10"
cat runs.csv

# csv, pipe-delimited, headers suppressed (-H)
stackql exec --output csv -f repos.psv -H -d="|" \
"SELECT name, visibility, archived FROM github.repos.repos WHERE org = 'stackql'"
head -5 repos.psv

# csv straight into column
stackql exec --output csv \
"SELECT name, stars, forks FROM (SELECT name, stargazers_count AS stars, forks_count AS forks FROM github.repos.repos WHERE org = 'stackql' AND archived = 0 ORDER BY stargazers_count DESC) t LIMIT 5" | column -s, -t

# queries from a file (-i), csv to stdout
stackql exec -i primer/queries/workflow-runs.iql --output csv

# parameterized queries: jsonnet config (--iqldata) plus an external var (--var)
# vars.jsonnet renders the org and the IN list of key repos; the query templates them in
stackql exec -i primer/queries/branch-protection.iql \
  --iqldata primer/queries/vars.jsonnet \
  --var org=stackql --output csv

# same thing driven by an environment variable (GITHUB_ORG is in .env)
stackql exec -i primer/queries/branch-protection.iql \
  --iqldata primer/queries/vars.jsonnet \
  --var org=$GITHUB_ORG --output csv

# preview the rendered SQL without executing anything (--dryrun)
stackql exec -i primer/queries/branch-protection.iql \
  --iqldata primer/queries/vars.jsonnet \
  --var org=$GITHUB_ORG --dryrun --output text

# the bridge into acts 2 and 3: two planes in one statement, which EC2 instance
# does each A record in the zone point at (AWS + Cloudflare; the zone is looked
# up by name in a CTE, the zone id never appears)
stackql exec -i primer/queries/cross-provider.iql \
  --iqldata primer/queries/footprint-vars.jsonnet \
  --var zone=$DEMO_DOMAIN --var region=$AWS_REGION
