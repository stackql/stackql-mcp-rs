# Shared helpers for the act 1 scripts. Sourced, not executed.
# Slide: "How to use StackQL".

# Load .env from the repo root if present (credentials are optional).
_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
if [ -f "$_ROOT/.env" ]; then
  set -a
  # shellcheck disable=SC1091
  . "$_ROOT/.env"
  set +a
fi

# github runs unauthenticated unless STACKQL_GITHUB_USERNAME/PASSWORD are set.
# With creds the rate limit is 5000/hour instead of 60/hour; the queries are
# identical either way.
# --approot is explicit so the scripts behave the same from any directory.
STACKQL_ARGS=(--approot "${STACKQL_APPROOT:-$HOME/.stackql}")
if [ -z "${STACKQL_GITHUB_USERNAME:-}" ] || [ -z "${STACKQL_GITHUB_PASSWORD:-}" ]; then
  STACKQL_ARGS+=(--auth '{"github": {"type": "null_auth"}}')
fi

# Print the command, then run it. The room reads what runs.
run() {
  printf '\n$ %s\n' "$*"
  "$@"
}

# The SQL in a query file: comments and blank lines stripped, one line, no
# trailing semicolon (exec takes a single statement).
sql_of() {
  grep -v '^--' "$_ROOT/primer/queries/$1" | sed '/^$/d' | tr '\n' ' ' \
    | sed 's/  */ /g; s/[; ]*$//'
}

# Run one .iql file through stackql exec with the given extra flags. The
# echoed command goes to stderr so stdout stays pipeable.
q() {
  local file="$1"; shift
  local sql
  sql="$(sql_of "$file")"
  printf '\n$ stackql exec %s "%s"\n' "$*" "$sql" >&2
  stackql exec "${STACKQL_ARGS[@]}" "$@" "$sql"
}
