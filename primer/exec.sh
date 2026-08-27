# =============================================================================
# stackql exec - one-shot queries with output options
# copy and paste one block at a time (not a batch script)
# Act 1, slide "How to use StackQL" (Interactive Analytics row)
# prereqs: AWS_*, CLOUDFLARE_*, AWS_REGION and DEMO_DOMAIN exported from .env
# (set -a; . ./.env; set +a)
# Windows builds default --approot to the cwd: add --approot ~/.stackql there
# =============================================================================

# default output (table) to stdout: compute inventory, JSON state unpacked
stackql exec \
"SELECT instance_id, instance_type, JSON_EXTRACT(state, '$.name') AS state, public_ip_address AS ip FROM aws.ec2.instances WHERE region = '$AWS_REGION'"

# json to stdout (pipe into jq): the edge, plan unpacked from JSON
stackql exec --output json \
"SELECT name, status, JSON_EXTRACT(plan, '$.name') AS plan FROM cloudflare.zones.zones" | jq .

# jsonl (one object per line) written to a file: every rule open to the world
stackql exec --output jsonl -f exposure.jsonl \
"SELECT group_id, ip_protocol, from_port, to_port FROM aws.ec2.security_group_rules WHERE region = '$AWS_REGION' AND is_egress = 0 AND cidr_ipv_4 = '0.0.0.0/0'"
head -5 exposure.jsonl

# csv with headers to a file: volumes with a window function alongside
stackql exec --output csv -f volumes.csv \
"SELECT volume_id, size, volume_type, state, SUM(size) OVER (PARTITION BY state) AS gib_in_state FROM aws.ec2.volumes WHERE region = '$AWS_REGION'"
cat volumes.csv

# csv, pipe-delimited, headers suppressed (-H): the zone's records
stackql exec --output csv -f records.psv -H -d="|" \
"SELECT r.type, r.name, r.content FROM cloudflare.zones.zones z JOIN cloudflare.dns.zones_dns_records r ON r.zone_id = z.id WHERE z.name = '$DEMO_DOMAIN'"
cat records.psv

# csv straight into column
stackql exec --output csv \
"SELECT instance_id, instance_type, JSON_EXTRACT(state, '$.name') AS state, public_ip_address AS ip FROM aws.ec2.instances WHERE region = '$AWS_REGION'" | column -s, -t

# queries from a file (-i), csv to stdout: the risky-port exposure check
stackql exec -i primer/queries/exposure.iql --output csv

# parameterized queries: jsonnet config (--iqldata) plus an external var (--var)
# vars.jsonnet computes the Cost Explorer window from 'month'; the query templates it in
stackql exec -i primer/queries/finops.iql \
  --iqldata primer/queries/vars.jsonnet \
  --var month=2026-08 --output csv

# same thing driven by the clock
stackql exec -i primer/queries/finops.iql \
  --iqldata primer/queries/vars.jsonnet \
  --var month=$(date +%Y-%m) --output csv

# preview the rendered SQL without executing anything (--dryrun)
stackql exec -i primer/queries/finops.iql \
  --iqldata primer/queries/vars.jsonnet \
  --var month=$(date +%Y-%m) --dryrun --output text

# the bridge into acts 2 and 3: two planes in one statement, which EC2 instance
# does each A record in the zone point at (AWS + Cloudflare; the zone is looked
# up by name in a CTE, the zone id never appears)
stackql exec -i primer/queries/cross-provider.iql \
  --iqldata primer/queries/footprint-vars.jsonnet \
  --var zone=$DEMO_DOMAIN --var region=$AWS_REGION
