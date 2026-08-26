# =============================================================================
# stackql srv - a postgres-wire-protocol server over the same engine
# copy and paste one block at a time (not a batch script)
# Act 1, slide "How to use StackQL" (Application & Data Integration row)
# any postgres client works: psql, DBeaver, pgwire-lite (Node), pystackql (server_mode)
# Windows builds default --approot to the cwd: add --approot ~/.stackql to srv
# =============================================================================

# start the server in the background (default port 5466)
nohup stackql srv --pgsrv.port 5466 > stackql-srv.log 2>&1 &

# check it is up
psql -h localhost -p 5466 -U stackql -d stackql -c "SHOW PROVIDERS"

# one-shot query from psql
psql -h localhost -p 5466 -U stackql -d stackql \
-c "SELECT visibility, archived, COUNT(*) AS repos FROM github.repos.repos WHERE org = 'stackql' GROUP BY visibility, archived"

# expanded output for wide rows (psql \x)
psql -h localhost -p 5466 -U stackql -d stackql -x \
-c "SELECT name, description, language, stargazers_count, forks_count, default_branch, pushed_at FROM github.repos.repos WHERE org = 'stackql' AND name = 'stackql'"

# csv straight out of psql
psql -h localhost -p 5466 -U stackql -d stackql --csv \
-c "SELECT name, head_branch, conclusion, created_at FROM (SELECT name, head_branch, conclusion, created_at FROM github.actions.workflow_runs WHERE owner = 'stackql' AND repo = 'stackql' ORDER BY created_at DESC) t LIMIT 5"

# interactive session (then paste queries from shell.iql)
psql -h localhost -p 5466 -U stackql -d stackql

# a Node app over the same wire protocol (deps first: cd primer/pgwire-lite-app && npm install; prewarm.sh does it)
node primer/pgwire-lite-app/app.js

# pystackql can talk to the server too (server_mode) instead of driving its own binary
python -c "from pystackql import StackQL; print(StackQL(server_mode=True, output='pandas').execute(\"SELECT visibility, archived, COUNT(*) AS repos FROM github.repos.repos WHERE org = 'stackql' GROUP BY visibility, archived\"))"

# stop the server
pkill -f "stackql srv"
