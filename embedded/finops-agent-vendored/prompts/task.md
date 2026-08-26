Today is {{ TODAY }}. Produce the month-to-date FinOps report for our AWS
account, compute in region `{{ AWS_REGION }}`, using the stackql tools. Use
the discovery tools before writing SQL rather than guessing table or column
names.

1. Spend: month-to-date unblended cost by service from Cost Explorer
   (aws.ce.cost_and_usages, region 'us-east-1', TimePeriod from the first of
   this month to today, Granularity MONTHLY, Metrics UnblendedCost, grouped
   by the SERVICE dimension). Rank the services, largest first, and give the
   total.
2. Compute inventory: instances in `{{ AWS_REGION }}` (aws.ec2.instances)
   with state, instance type and whether they carry `owner` and
   `cost-centre` tags (aws.ec2.tags, or the tags column). Untagged running
   compute is unallocated spend.
3. Waste: stopped instances (their EBS volumes still bill), volumes with
   state 'available' (aws.ec2.volumes: attached to nothing, still billing),
   and Elastic IPs with no association (aws.ec2.addresses). Estimate the
   monthly cost of the waste with these rates, do not look prices up: EBS
   gp3 USD 0.096 per GiB-month, gp2 USD 0.12, a public IPv4 address USD
   3.65 a month whether or not it is in use.

Budget: about a dozen tool calls. Discover a resource once, then query it;
one query per check is enough. Do not use the pricing or billing services.

Finish with a short plain-text report: spend by service (top services and
the total), the compute inventory with its tagging gaps, a one-line waste
verdict with the estimated monthly figure, and one recommendation.
