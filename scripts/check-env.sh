#!/usr/bin/env bash
# Confirms the demo machine has what the three acts need. Run before prewarm.
set -euo pipefail
. "$(dirname "$0")/_env.sh"

ok=0
need() {
  local bin="$1" note="$2"
  if command -v "$bin" >/dev/null 2>&1; then
    printf '  ok      %-16s %s\n' "$bin" "$($bin --version 2>/dev/null | head -1)"
  else
    printf '  MISSING %-16s %s\n' "$bin" "$note"; ok=1
  fi
}
want() {
  local bin="$1" note="$2"
  if command -v "$bin" >/dev/null 2>&1; then
    printf '  ok      %-16s\n' "$bin"
  else
    printf '  (opt)   %-16s %s\n' "$bin" "$note"
  fi
}

echo "required:"
need stackql        "https://stackql.io/docs/installing-stackql"
need stackql-deploy "cargo install stackql-deploy, or get-stackql-deploy.io"
need cargo          "rustup: https://rustup.rs (MSRV 1.88)"
echo "optional:"
want psql   "act 1 step 3 (stackql srv); brew install libpq"
want jq     "act 1 pipes"
want column "act 1 pipes (util-linux / bsdmainutils)"

echo "credentials (.env):"
for v in AWS_ACCESS_KEY_ID AWS_SECRET_ACCESS_KEY AWS_REGION AWS_VPC_ID AWS_SUBNET_ID AWS_AMI_ID \
         CLOUDFLARE_API_TOKEN CLOUDFLARE_ZONE_ID ANTHROPIC_API_KEY \
         STACKQL_GITHUB_USERNAME STACKQL_GITHUB_PASSWORD; do
  if [ -n "${!v:-}" ]; then printf '  set     %s\n' "$v"; else printf '  unset   %s\n' "$v"; fi
done
printf '  service %s.%s in %s\n' "${DEMO_HOST:-rust-demo}" "${DEMO_DOMAIN:-stackql.xyz}" "${AWS_REGION:-?}"
printf '  github  %s/%s (zero-credential variant)\n' "$GITHUB_ORG" "$GITHUB_REPO"

echo "rust toolchain:"
rv="$(rustc --version 2>/dev/null | awk '{print $2}')"
printf '  rustc %s (need >= 1.88)\n' "${rv:-none}"

exit $ok
