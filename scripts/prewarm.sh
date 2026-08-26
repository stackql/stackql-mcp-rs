#!/usr/bin/env bash
# Leaves the machine able to run every act with no network: providers pulled,
# the sidecar bundle cached, the vendored binaries built, cargo deps in the
# local registry, and one query of each kind already answered once.
# Run the night before. Takes a few minutes the first time.
set -euo pipefail
. "$(dirname "$0")/_env.sh"
cd "$_ROOT"

echo "== 1. providers (act 1 github; acts 2 and 3 aws + awscc + cloudflare)"
for p in github aws awscc cloudflare; do
  run stackql exec "${STACKQL_ARGS[@]}" "REGISTRY PULL $p" >/dev/null
done

echo "== 2. act 1: answer each primer query file once (proves the path), install the app deps"
run stackql exec "${STACKQL_ARGS[@]}" -o csv -i primer/queries/workflow-runs.iql | head -3
run stackql exec "${STACKQL_ARGS[@]}" -o csv -i primer/queries/branch-protection.iql \
  --iqldata primer/queries/vars.jsonnet --var org="$GITHUB_ORG" | head -3
if command -v npm >/dev/null 2>&1; then
  (cd primer/pgwire-lite-app && run npm install --no-audit --no-fund >/dev/null)
else
  echo "  npm not on PATH: skipping primer/pgwire-lite-app (the Node app in act 1 step 3)"
fi
PY=""
for cand in python3 python; do
  if command -v "$cand" >/dev/null 2>&1 && "$cand" -c 'import pystackql, pandas' >/dev/null 2>&1; then PY="$cand"; break; fi
done
if [ -n "$PY" ]; then
  run "$PY" primer/pystackql-app/app.py | head -3    # downloads the pystackql binary on first run
else
  echo "  pystackql/pandas not importable: pip install pystackql pandas (the Python app in act 1 step 4)"
fi

echo "== 3. act 2: stackql-deploy dry-runs (no credentials needed)"
run stackql-deploy build stacks/service-footprint dev --env-file .env --dry-run >/dev/null
run stackql-deploy build stacks/golden-path dev --env-file .env --dry-run >/dev/null

echo "== 4. act 3: build both agents (finops-agent-vendored's build.rs fetches the bundle), then preflight each"
(cd embedded && run cargo build --release)
run ./embedded/target/release/sre-agent-sidecar --check >/dev/null       # sidecar: downloads and caches the server
run ./embedded/target/release/finops-agent-vendored --check >/dev/null    # vendored: extracts the server from the binary

echo "== 5. sizes"
ls -lh embedded/target/release/sre-agent-sidecar embedded/target/release/finops-agent-vendored 2>/dev/null \
  | awk '{print "  " $5 "  " $9}'
echo "== cache"
ls -R "${STACKQL_APPROOT:-$HOME/.stackql}/mcp-server-bin" | head -20

echo
echo "prewarm complete. With .env exported, converge the estate and warm the models:"
echo "  stackql-deploy build stacks/service-footprint dev --env-file .env"
echo "  ./embedded/target/release/sre-agent-sidecar"
echo "  ./embedded/target/release/finops-agent-vendored"
