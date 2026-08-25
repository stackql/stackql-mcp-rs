# Sourced by the scripts: repo root, .env, stackql args.
_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
if [ -f "$_ROOT/.env" ]; then
  set -a
  # shellcheck disable=SC1091
  . "$_ROOT/.env"
  set +a
fi
export GITHUB_ORG="${GITHUB_ORG:-stackql}"
export GITHUB_REPO="${GITHUB_REPO:-rust-embedded-mcp-with-stackql}"

STACKQL_ARGS=(--approot "${STACKQL_APPROOT:-$HOME/.stackql}")
if [ -z "${STACKQL_GITHUB_USERNAME:-}" ] || [ -z "${STACKQL_GITHUB_PASSWORD:-}" ]; then
  STACKQL_ARGS+=(--auth '{"github": {"type": "null_auth"}}')
fi

run() {
  printf '\n$ %s\n' "$*" >&2
  "$@"
}
