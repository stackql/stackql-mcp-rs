#!/usr/bin/env bash
# Act 1, step 3: `stackql srv` speaks the Postgres wire protocol; connect
# with psql (or any pg client / BI tool) and run the same query.
# Slide: "How to use StackQL" (Application & Data Integration row).
set -euo pipefail
. "$(dirname "$0")/_lib.sh"

PORT="${STACKQL_SRV_PORT:-5466}"

run stackql srv "${STACKQL_ARGS[@]}" --pgsrv.port="$PORT" &
SRV_PID=$!
trap 'kill "$SRV_PID" 2>/dev/null || true' EXIT
sleep 3

run psql -h localhost -p "$PORT" -U stackql -c "$(grep -v '^--' "$_ROOT/primer/queries/02-inventory.iql")"
run psql -h localhost -p "$PORT" -U stackql -c "$(grep -v '^--' "$_ROOT/primer/queries/03-top-repos.iql")"

echo
echo "Server still running on port $PORT for an interactive psql session:"
echo "  psql -h localhost -p $PORT -U stackql"
echo "Press Enter to stop the server."
read -r _
