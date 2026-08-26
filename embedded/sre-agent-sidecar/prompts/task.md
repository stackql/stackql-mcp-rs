Run the morning assurance sweep for the service `{{ DEMO_HOST }}`: one EC2
instance in AWS region `{{ AWS_REGION }}`, reached at
`{{ DEMO_HOST }}.{{ DEMO_DOMAIN }}`, an A record in Cloudflare zone
`{{ CLOUDFLARE_ZONE_ID }}`. Read actual state from both providers with the
stackql tools; there is no state file. Use the discovery tools before writing
SQL rather than guessing table or column names.

1. Service health: find the instance and its security group by the tag
   `service = {{ DEMO_HOST }}` (aws.ec2.tags with region, key and value;
   resource_type is instance or security-group), then read the instance
   (aws.ec2.instances) for state, instance type, public IP and launch time.
   Ignore instances that are terminated or shutting-down; AWS keeps their
   tags readable for a while. Exactly one live instance is expected.
2. Network exposure: inbound rules on that security group open to 0.0.0.0/0
   (aws.ec2.security_group_rules with region and group_id, is_egress = 0).
   Ports 80 and 443 are expected; anything else, port 22 in particular, is a
   finding.
3. Edge: the A record `{{ DEMO_HOST }}.{{ DEMO_DOMAIN }}` exists and its
   content is the instance's public IP (cloudflare.dns.zones_dns_records with
   zone_id). Any other A record in the zone whose name starts with
   `{{ DEMO_HOST }}` and does not point at a running instance is dangling.
4. Governance: the instance carries `owner` and `cost-centre` tags.

Finish with a short plain-text report: one line per check, PASS or
ATTENTION, with one line of evidence each. For each ATTENTION, add the single
SQL statement that would fix it (awscc for AWS writes, cloudflare.dns.records
for Cloudflare deletes), clearly marked as not run: this session is
read-only.
