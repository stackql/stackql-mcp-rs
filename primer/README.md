# Act 1 - StackQL primer

One engine, several ways to consume it. Slides: STACKQL >>, Cloud Providers as Data Sources, How to use StackQL. Everything here runs against public GitHub data with zero credentials; if `STACKQL_GITHUB_USERNAME` / `STACKQL_GITHUB_PASSWORD` are set in `.env` the same queries run authenticated, which lifts the GitHub rate limit from 60/hour to 5000/hour. The cross-provider query (Cloudflare DNS records joined to the EC2 instances they point at) needs the act 2/3 credentials and is the bridge into the next two acts; it returns a row per A record that points at a running instance, so it needs `stacks/aws-webserver` (native `aws` provider, four resources) or `stacks/service-footprint` built first, one row each.

The `.sh` and `.iql` files are paste blocks, not batch scripts: copy one block at a time into a shell (or into `stackql shell`) from the repo root with `.env` exported (`set -a; . ./.env; set +a`).

| File | Shows |
|---|---|
| `shell.iql` | `stackql shell`: `REGISTRY PULL`, `SHOW PROVIDERS`, `SHOW SERVICES`, `SHOW RESOURCES`, `SHOW EXTENDED METHODS`, `DESCRIBE EXTENDED`, then SELECTs, a JOIN, a cross-provider JOIN, and the mutation verbs |
| `exec.sh` | `stackql exec`: `--output table`, `json`, `jsonl`, `csv`, to stdout or a file (`-f`), `-H` (no header), `-d` (delimiter), piped into `jq` and `column`, queries from a file (`-i`), jsonnet templating (`--iqldata` + `--var`), `--dryrun` |
| `srv.sh` | `stackql srv` (Postgres wire protocol) with `psql`, the Node app, and pystackql in `server_mode` |
| `queries/` | the `.iql` files `exec.sh` reads with `-i`, and the jsonnet that parameterises them |
| `pystackql-app/` | Python: `pystackql` driving the engine, results as pandas DataFrames (`pip install pystackql pandas`, then `python primer/pystackql-app/app.py`) |
| `pgwire-lite-app/` | Node: `@stackql/pgwire-lite` talking to `stackql srv` (`cd primer/pgwire-lite-app && npm install` once, then `node primer/pgwire-lite-app/app.js` from the repo root) |

Notes that matter on the day:

- Fan-out: a JOIN or an `IN (...)` on a key column becomes one API call per key. `branch-protection.iql` is written to make three calls, not one per repo in the org.
- `ORDER BY` with `LIMIT` in the same SELECT applies the limit first; wrap the ordered query in a subquery, as every ordered query here does.
- Booleans come back as `true`/`false` text but are compared as `0`/`1` (`archived = 0`). pystackql hands every column back as text; `app.py` converts the counters with `pd.to_numeric` before aggregating.
- Windows builds of `stackql` default `--approot` to the current directory; add `--approot ~/.stackql` to each `stackql` command there (the executed scripts in `scripts/` already do). On macOS and Linux the default is `~/.stackql`.
- `exec.sh` writes `repos.jsonl`, `runs.csv` and `repos.psv` into the repo root; they are gitignored.
- Column widths are chosen to fit an 80x24 terminal.
