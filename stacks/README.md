# Act 2 - stackql-deploy, Rust native

`stackql-deploy` is the declarative side of StackQL: a stack is a manifest plus `.iql` resource files, and `build` / `test` / `teardown` converge live state onto it with no state file. The current release is a Rust binary (`stackql-deploy info` shows the version and no Python anywhere).

The stack here, `golden-path/`, declares what a repository on our golden path looks like:

| Resource | What it converges |
|---|---|
| `repo_topics` | the repo carries the topics `stackql`, `rust`, `mcp`, `golden-path` |
| `label_needs_triage`, `label_security`, `label_agent_fixed` | a standard label set (one resource each, all sharing `resources/label.iql`) |
| `default_branch_ruleset` | a ruleset on the default branch that blocks deletion and force pushes |
| `conformance` | a `query` resource: one row summarising the checks, exported as `golden_path_ok` |

It targets `GITHUB_ORG` / `GITHUB_REPO` from `.env` (defaults in `.env.example` point at this repo). Act 3's agent, `steward`, watches the same repo for drift, so the two acts tell one story: declare the golden path, converge, then let an agent detect and repair drift against actual state.

Requirements: `stackql-deploy` on PATH (`cargo install stackql-deploy` or the installer at get-stackql-deploy.io), and `STACKQL_GITHUB_USERNAME` / `STACKQL_GITHUB_PASSWORD` in `.env` with a token that can administer the target repo. `--dry-run` needs no credentials.

Commands, run from the repo root:

```sh
stackql-deploy info
stackql-deploy build stacks/golden-path dev --env-file .env --dry-run --show-queries
stackql-deploy build stacks/golden-path dev --env-file .env
stackql-deploy test  stacks/golden-path dev --env-file .env
stackql-deploy teardown stacks/golden-path dev --env-file .env
```

Argument order is `<command> <stack_dir> <stack_env> [flags]`. `--show-queries` prints the rendered SQL before each call.

Authoring notes (github provider, current engine):

- Body parameters are plain column names in `INSERT`: `INSERT INTO github.issues.labels (owner, repo, name, color, description) SELECT ...`.
- `REPLACE github.repos.topics SET names = '["a","b"]' WHERE owner = ... AND repo = ...` sends a JSON array.
- Classic branch protection (`github.repos.branch_protection`) needs boolean body fields, which `REPLACE ... SET x = false` cannot express yet; rulesets (`github.repos.rules`) take JSON strings and work. The stack uses a ruleset.
- The `exists` query on the ruleset selects `id AS ruleset_id`; the captured value is available as `{{ this.ruleset_id }}` for `delete`.
- Environment variables from `--env-file` are referenced directly in the manifest as `{{ GITHUB_ORG }}`.
- Manifest props reach templates as JSON strings: `{{ topics }}` inlines the array, `{{ topics | sql_list }}` renders `('a','b')`, `{{ topics | from_json | length }}` counts.
- One statement per anchor. Multi-statement anchors only run their first statement over the wire, so each label is its own resource.
