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

echo "== 2. act 1: answer each primer query once (warms nothing on the server, but proves the path)"
for f in 02-inventory 03-top-repos 04-join 05-workflow-runs; do
  run stackql exec "${STACKQL_ARGS[@]}" -o csv "$(grep -v '^--' primer/queries/$f.iql | sed '/^$/d' | tr '\n' ' ' | sed 's/[; ]*$//')" | head -3
done

echo "== 3. act 2: stackql-deploy dry-runs (no credentials needed)"
run stackql-deploy build stacks/service-footprint dev --env-file .env --dry-run >/dev/null
run stackql-deploy build stacks/golden-path dev --env-file .env --dry-run >/dev/null

echo "== 4. act 3: build everything, sidecar bundle cached on first run"
(cd embedded && run cargo build --release -p minimal -p steward)
run ./embedded/target/release/minimal >/dev/null
run ./embedded/target/release/steward preflight

echo "== 5. act 3: vendored builds (bundle fetched at build time, then embedded)"
(cd embedded && run cargo build --release -p minimal-vendored)
(cd embedded && run cargo build --release -p steward --features vendored --target-dir target/vendored)
run ./embedded/target/release/minimal-vendored >/dev/null
run ./embedded/target/vendored/release/steward preflight

echo "== 6. sizes"
ls -lh embedded/target/release/minimal embedded/target/release/minimal-vendored \
       embedded/target/release/steward embedded/target/vendored/release/steward 2>/dev/null \
  | awk '{print "  " $5 "  " $9}'
echo "== cache"
ls -R "${STACKQL_APPROOT:-$HOME/.stackql}/mcp-server-bin" | head -20

echo
echo "prewarm complete. With .env exported, converge the estate and warm the model:"
echo "  stackql-deploy build stacks/service-footprint dev --env-file .env"
echo "  ./embedded/target/release/steward check"
