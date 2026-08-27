# =============================================================================
# stackql srv - a postgres-wire-protocol server over the same engine
# copy and paste one block at a time (not a batch script)
# Act 1, slide "How to use StackQL" (Application & Data Integration row)
# any postgres client works: psql, DBeaver, pgwire-lite (Node), pystackql (server_mode)
# prereqs: AWS_*, CLOUDFLARE_*, AWS_REGION and DEMO_DOMAIN exported from .env
# (the server reads provider credentials from its environment)
# Windows builds default --approot to the cwd: add --approot ~/.stackql to srv
# =============================================================================

# start the server in the background (default port 5466)
nohup stackql srv --pgsrv.port 5466 > stackql-srv.log 2>&1 &

# check it is up
psql -h localhost -p 5466 -U stackql -d stackql -c "SHOW PROVIDERS"

# one-shot query from psql: compute inventory
psql -h localhost -p 5466 -U stackql -d stackql \
-c "SELECT instance_id, instance_type, JSON_EXTRACT(state, '$.name') AS state, public_ip_address AS ip FROM aws.ec2.instances WHERE region = '$AWS_REGION'"

# expanded output for wide rows (psql \x): the running instances, JSON columns and all
psql -h localhost -p 5466 -U stackql -d stackql -x \
-c "SELECT instance_id, instance_type, public_ip_address, launch_time, security_groups, tags FROM aws.ec2.instances WHERE region = '$AWS_REGION' AND JSON_EXTRACT(state, '$.name') = 'running'"

# csv straight out of psql: the zone's records
psql -h localhost -p 5466 -U stackql -d stackql --csv \
-c "SELECT r.type, r.name, r.content FROM cloudflare.zones.zones z JOIN cloudflare.dns.zones_dns_records r ON r.zone_id = z.id WHERE z.name = '$DEMO_DOMAIN'"

# interactive session (then paste queries from shell.iql)
psql -h localhost -p 5466 -U stackql -d stackql

# a Node app over the same wire protocol (deps first: cd primer/pgwire-lite-app && npm install)
node primer/pgwire-lite-app/app.js

# pystackql can talk to the server too (server_mode) instead of driving its own binary
python -c "from pystackql import StackQL; print(StackQL(server_mode=True, output='pandas').execute(\"SELECT JSON_EXTRACT(state, '\$.name') AS state, COUNT(*) AS instances FROM aws.ec2.instances WHERE region = '$AWS_REGION' GROUP BY 1\"))"

# stop the server
pkill -f "stackql srv"
