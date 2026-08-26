# Act 2 - stackql-deploy, Rust native

`stackql-deploy` is the declarative side of StackQL: a stack is a manifest plus `.iql` resource files, and `build` / `test` / `teardown` converge live state onto it with no state file. The current release is a Rust binary (`stackql-deploy info` shows the version and no Python anywhere). Argument order is `<command> <stack_dir> <stack_env> [flags]`; `--show-queries` prints the rendered SQL before each call; it needs `stackql` on PATH (`cargo install stackql-deploy` or get-stackql-deploy.io).

Three stacks. `service-footprint/` is the one in the talk; `aws-webserver/` is the same idea done with the native `aws` provider only (four resources, no tags, the stack that backs the act 1 cross-provider query); `golden-path/` is the GitHub variant for people without cloud credentials.

## aws-webserver

A `t3.micro` running httpd in the default VPC, its security group, and an A record `webserver-<env>.stackql.xyz` pointing at its public IP, written entirely with `aws` (EC2 Query API) and `cloudflare`. Identity by natural key (group name, security-group membership, record name) because the native provider cannot tag. Clean build about 25 s, re-converge 3 s. See [aws-webserver/README.md](aws-webserver/README.md) for the resource table and the list of what the native provider can and cannot do (no tags, no ingress rules, no subnet public-IP attribute, so port 80 stays closed; the query still works).

```sh
stackql-deploy build stacks/aws-webserver dev --env-file .env --dry-run --show-queries
stackql-deploy build stacks/aws-webserver dev --env-file .env
stackql-deploy test  stacks/aws-webserver dev --env-file .env
stackql-deploy teardown stacks/aws-webserver dev --env-file .env
```

## service-footprint

One service's footprint across two planes, as data:

| Resource | Provider | What it converges |
|---|---|---|
| `web_sg` | `awscc` | security group, 443 and 80 from anywhere, nothing else inbound, tagged |
| `web_server` | `awscc` | one `t3.micro` running httpd, tagged `service`, `owner`, `cost-centre` |
| `web_server_ip` | `aws` | `query` resource exporting the instance's public IP |
| `dns_record` | `cloudflare` | A record `rust-demo.stackql.xyz` pointing at that IP |
| `conformance` | both | `query` resource: no port 22 rule, owner tag present, record matches; exported as `footprint_ok` |

Identity and desired state are separate tag sets on purpose: `identity_tags` (`service`, `stackql:stack-name`, `stackql:stack-env`) find the resources in the `exists` queries and never drift; `owner` and `cost-centre` are desired state that `update` restores. Mixing them makes a stripped tag look like a missing resource, and `build` creates a second one.

Needs in `.env`: `AWS_*`, `AWS_VPC_ID`, `AWS_SUBNET_ID` (auto-assign public IP), `AWS_AMI_ID`, `CLOUDFLARE_API_TOKEN` (Zone:DNS:Edit on the zone), `CLOUDFLARE_ZONE_ID`, `DEMO_HOST`, `DEMO_DOMAIN`. Export them (`set -a; . ./.env; set +a`) as well as passing `--env-file`: the file feeds the templates, the environment feeds the stackql server's provider auth.

```sh
stackql-deploy build stacks/service-footprint dev --env-file .env --dry-run --show-queries
stackql-deploy build stacks/service-footprint dev --env-file .env      # about 30 s from nothing
stackql-deploy test  stacks/service-footprint dev --env-file .env
stackql-deploy teardown stacks/service-footprint dev --env-file .env   # the SG delete retries until the instance has gone
```

Measured: clean build 27 s, re-converge after drift about 40 s (the instance statecheck retries are the wait), teardown about 40 s.

Authoring notes (aws, awscc, cloudflare):

- Read with `aws` (describe views: `aws.ec2.instances`, `aws.ec2.tags`, `aws.ec2.security_group_rules`), write with `awscc` (Cloud Control): `INSERT` creates, `UPDATE ... SET PatchDocument = string('[json patch]') WHERE Identifier = ...` modifies, `DELETE ... WHERE Identifier = ...` removes. The native `aws` EC2 Query-API writes work for scalar parameters (`create_security_group`, `run_instances` with a single `SecurityGroupId`, `terminate_instances`, `create_vpc`, `RETURNING group_id`) but not for list or nested ones (`Tag`, `TagSpecification`, `IpPermissions`, `MapPublicIpOnLaunch`): the engine sends them as one JSON string where the API wants `Name.1.Field=...`, and AWS answers `InvalidRequest`. `aws-webserver/` is built inside those limits.
- `cloudflare.dns.zones_dns_records` lists and creates records; `cloudflare.dns.records` edits and deletes by `dns_record_id`. `cloudflare.dns.dns_records` maps to the export endpoint and is not a row source.
- The engine sends `UPDATE ... SET ttl = 300` as the string `"300"`, which Cloudflare rejects, and `ttl` is required on the edit method, so a record cannot be re-pointed by UPDATE today; the stack has no `update` anchor for `dns_record`; a re-point is delete plus insert.
- `awscc` mutations are asynchronous; statechecks and deletes carry retries.

## golden-path (GitHub)

Declares what a repository on our golden path looks like:

| Resource | What it converges |
|---|---|
| `repo_topics` | the repo carries the topics `stackql`, `rust`, `mcp`, `golden-path` |
| `label_needs_triage`, `label_security`, `label_agent_fixed` | a standard label set (one resource each, all sharing `resources/label.iql`) |
| `default_branch_ruleset` | a ruleset on the default branch that blocks deletion and force pushes |
| `conformance` | a `query` resource: one row summarising the checks, exported as `golden_path_ok` |

It targets `GITHUB_ORG` / `GITHUB_REPO` from `.env` (defaults in `.env.example` point at this repo). `scripts/drift.sh` introduces drift for a re-converge demo. Needs `STACKQL_GITHUB_USERNAME` / `STACKQL_GITHUB_PASSWORD` in `.env` with a token that can administer the target repo for `build`; `--dry-run` and reads need no credentials.

```sh
stackql-deploy info
stackql-deploy build stacks/golden-path dev --env-file .env --dry-run --show-queries
stackql-deploy build stacks/golden-path dev --env-file .env
stackql-deploy test  stacks/golden-path dev --env-file .env
stackql-deploy teardown stacks/golden-path dev --env-file .env
```

Authoring notes (github provider):

- Body parameters are plain column names in `INSERT`: `INSERT INTO github.issues.labels (owner, repo, name, color, description) SELECT ...`.
- `REPLACE github.repos.topics SET names = '["a","b"]' WHERE owner = ... AND repo = ...` sends a JSON array.
- Classic branch protection (`github.repos.branch_protection`) needs boolean body fields, which `REPLACE ... SET x = false` cannot express yet; rulesets (`github.repos.rules`) take JSON strings and work. The stack uses a ruleset.
- The `exists` query on the ruleset selects `id AS ruleset_id`; the captured value is available as `{{ this.ruleset_id }}` for `delete`.
- Environment variables from `--env-file` are referenced directly in the manifest as `{{ GITHUB_ORG }}`.
- Manifest props reach templates as JSON strings: `{{ topics }}` inlines the array, `{{ topics | sql_list }}` renders `('a','b')`, `{{ topics | from_json | length }}` counts.
- One statement per anchor. Multi-statement anchors only run their first statement over the wire, so each label is its own resource.
