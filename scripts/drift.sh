#!/usr/bin/env bash
# Act 3: "someone changed it in the console". Introduces two pieces of drift
# on the target repo for a re-converge demo:
#   - deletes the `security` label
#   - drops `golden-path` from the topics
# Needs STACKQL_GITHUB_USERNAME / STACKQL_GITHUB_PASSWORD in .env.
# Undo: stackql-deploy build stacks/golden-path dev --env-file .env
set -euo pipefail
. "$(dirname "$0")/_env.sh"

if [ -z "${STACKQL_GITHUB_USERNAME:-}" ] || [ -z "${STACKQL_GITHUB_PASSWORD:-}" ]; then
  echo "drift.sh needs STACKQL_GITHUB_USERNAME / STACKQL_GITHUB_PASSWORD in .env" >&2
  exit 1
fi

run stackql exec "${STACKQL_ARGS[@]}" \
  "DELETE FROM github.issues.labels WHERE owner = '$GITHUB_ORG' AND repo = '$GITHUB_REPO' AND name = 'security'"

run stackql exec "${STACKQL_ARGS[@]}" \
  "REPLACE github.repos.topics SET names = '[\"stackql\",\"rust\",\"mcp\"]' WHERE owner = '$GITHUB_ORG' AND repo = '$GITHUB_REPO'"

echo
echo "drift introduced on $GITHUB_ORG/$GITHUB_REPO: label 'security' gone, topic 'golden-path' gone."
