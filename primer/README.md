# Act 1 - StackQL primer

One engine, several ways to consume it. Everything here runs against public GitHub data with zero credentials (`github` provider, `null_auth`). If `STACKQL_GITHUB_USERNAME` / `STACKQL_GITHUB_PASSWORD` are set in `.env` the same queries run authenticated, which lifts the GitHub rate limit from 60/hour to 5000/hour. Set them before a live demo.

| Script | Shows |
|---|---|
| `01-shell.sh` | `stackql shell`: `SHOW PROVIDERS`, `SHOW SERVICES`, `SHOW RESOURCES`, `SHOW METHODS`, `DESCRIBE`, then SELECTs and a JOIN pasted from `queries/` |
| `02-exec-formats.sh` | `stackql exec` with `--output table`, `json`, `jsonl`, `csv`, `-H` (no header), `-d` (delimiter), piped into `column` and `jq` |
| `03-srv-psql.sh` | `stackql srv` (Postgres wire protocol) and `psql` running the same query |

Query files in `queries/` are numbered in demo order and each carries a comment saying which slide it belongs to. `06-cross-provider.iql` needs AWS credentials and is an optional flourish.

Notes that matter on the day:

- Fan-out: a JOIN or an `IN (...)` on a key column becomes one API call per key. `04-join.iql` is written to make three calls, not forty.
- `ORDER BY` with `LIMIT` in the same SELECT applies the limit first; wrap the ordered query in a subquery, as `03-top-repos.iql` does.
- Booleans come back as `true`/`false` text but are compared as `0`/`1` (`archived = 0`).
- Column widths are chosen to fit an 80x24 terminal.
