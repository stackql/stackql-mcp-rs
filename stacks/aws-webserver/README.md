# aws-webserver

A web server in the default VPC with an A record on the demo zone, built with the native `aws` provider (the EC2 Query API, no `awscc`) and the `cloudflare` provider. It is the stack that backs the act 1 cross-provider query (`primer/queries/cross-provider.iql`): which EC2 instance does each A record in the zone point at.

| Resource | Provider | What it converges |
|---|---|---|
| `security_group` | `aws` | a security group named `aws-webserver-<env>-sg` in the default VPC; also how the stack finds its instance |
| `web_server` | `aws` | one `t3.micro` (Amazon Linux 2023, httpd from user data) in the default VPC subnet, member of that group |
| `web_server_ip` | `aws` | `query` resource exporting the instance's public IP |
| `dns_record` | `cloudflare` | A record `webserver-<env>.stackql.xyz` pointing at that IP |

Identity is by natural key, not tags: the group by name and VPC, the instance by subnet and security-group membership (`JSON_EXTRACT(security_groups, '$.item.groupId')`), the record by name and type.

Needs in `.env`: `AWS_*`, `AWS_VPC_ID`, `AWS_SUBNET_ID` (a subnet that auto-assigns public IPs; the default VPC's do), `AWS_AMI_ID`, `CLOUDFLARE_API_TOKEN`, `CLOUDFLARE_ZONE_ID`, `DEMO_DOMAIN`. Export them as well as passing `--env-file`.

```sh
set -a; . ./.env; set +a
stackql-deploy build stacks/aws-webserver dev --env-file .env --dry-run --show-queries
stackql-deploy build stacks/aws-webserver dev --env-file .env
stackql-deploy test  stacks/aws-webserver dev --env-file .env
stackql-deploy teardown stacks/aws-webserver dev --env-file .env
```

Then the query it exists for:

```sh
stackql exec -i primer/queries/cross-provider.iql --iqldata primer/queries/footprint-vars.jsonnet \
  --var zone=$DEMO_DOMAIN --var region=$AWS_REGION
```

## What the native aws provider can and cannot do here

The `aws` provider's EC2 resources call the EC2 Query API. Scalar parameters work as plain columns (`INSERT INTO aws.ec2.security_groups (GroupName, GroupDescription, VpcId, region) ...`, `INSERT INTO aws.ec2.instances (ImageId, InstanceType, MinCount, MaxCount, SubnetId, SecurityGroupId, UserData, region) ...`, `DELETE FROM aws.ec2.instances WHERE InstanceId = ...`), and `RETURNING <snake_case_column>` gives back ids (`RETURNING group_id`, `RETURNING internet_gateway_id`). List and nested parameters do not work in the current provider release: the engine sends them as a JSON string (`Tag=[{"Key":...}]`) where the Query API expects `Tag.1.Key=...&Tag.1.Value=...`, and AWS answers `InvalidRequest`. That rules out, with this provider alone:

- tags (`create_tags`, `TagSpecification` on any create), hence the natural-key identity above
- security group rules (`authorize_security_group_ingress` only accepts the `IpPermissions` list now; the legacy flat `CidrIp`/`FromPort` form is rejected by AWS with `UnknownParameter`), so port 80 is not opened and httpd is not reachable from the internet; the record and the query still work
- `modify_subnet_attribute` `MapPublicIpOnLaunch` (a nested `Value`), so a custom VPC's subnet cannot auto-assign public IPs; the instance goes into the default VPC subnet instead, which already does
- an Elastic IP as the alternative (`aws.ec2.address.allocate_address` is flat and works, but this account's EIP quota is full: `AddressLimitExceeded`)

The `service-footprint` stack next door does the same footprint with `awscc` writes (tags, ingress rules, patch updates) and `aws` reads; it is the one sre-agent-sidecar sweeps. This one exists to show the native provider doing a whole stack on its own, and to give the primer query a second row.
