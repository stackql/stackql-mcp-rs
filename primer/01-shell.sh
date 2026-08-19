#!/usr/bin/env bash
# Act 1, step 1: interactive discovery in `stackql shell`.
# Slide: "How to use StackQL" (Interactive Analytics row).
# Opens the shell; paste statements from queries/01-discovery.iql,
# then 02-inventory.iql and 04-join.iql.
set -euo pipefail
. "$(dirname "$0")/_lib.sh"

echo "Paste from primer/queries/01-discovery.iql, then 02, 03, 04. Ctrl-D to exit."
run stackql shell "${STACKQL_ARGS[@]}"
