#!/usr/bin/env bash
# Act 3: "someone changed it in the console". Introduces two pieces of drift
# on the target repo so steward has something to find and fix:
#   - deletes the `security` label
#   - drops `golden-path` from the topics
# Needs STACKQL_GITHUB_USERNAME / STACKQL_GITHUB_PASSWORD in .env.
# Undo: ./embedded/target/release/steward fix   (or: stackql-deploy build ...)
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
