#!/usr/bin/env bash
# Act 1, step 2: `stackql exec`, same query, every output format.
# Slide: "How to use StackQL" (Interactive Analytics row).
set -euo pipefail
. "$(dirname "$0")/_lib.sh"

q 03-top-repos.iql --output table
q 03-top-repos.iql --output json
q 03-top-repos.iql --output jsonl
q 03-top-repos.iql --output csv
q 03-top-repos.iql --output csv -H
q 03-top-repos.iql --output csv -d '|'

# Pipes: csv into column, json into jq (if installed).
printf '\n# csv into column\n' >&2
q 02-inventory.iql --output csv | column -s, -t
if command -v jq >/dev/null 2>&1; then
  printf '\n# json into jq\n' >&2
  q 03-top-repos.iql --output json | jq -r '.[] | [.name, .stars] | @tsv'
fi
