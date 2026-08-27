#!/usr/bin/env bash
# Act 3 (service-footprint): "someone changed it in the console". Introduces
# drift across both planes of the service so sre-agent-sidecar has something
# to find:
#   tag      drop the `owner` tag from the instance
#   ssh      open port 22 to the world on the security group
#   edge     delete the A record for the service
#   dangling add a second A record pointing at a stale IP
# Usage: stacks/service-footprint/drift.sh [tag] [ssh] [edge] [dangling]   (default: tag ssh edge)
# Needs AWS_* and CLOUDFLARE_API_TOKEN / CLOUDFLARE_ZONE_ID in .env (loaded here).
# Undo: stackql-deploy build stacks/service-footprint dev --env-file .env
#       (the dangling record is not in the stack; the RUNBOOK has the DELETE)
# Every statement is printed before it runs so the room can read it; echoes go
# to stderr so stdout stays pipeable.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
if [ -f "$ROOT/.env" ]; then
  set -a
  # shellcheck disable=SC1091
  . "$ROOT/.env"
  set +a
fi
: "${AWS_REGION:?set AWS_REGION in .env}"
: "${CLOUDFLARE_ZONE_ID:?set CLOUDFLARE_ZONE_ID in .env}"
DEMO_HOST="${DEMO_HOST:-rust-demo}"
DEMO_DOMAIN="${DEMO_DOMAIN:-stackql.xyz}"
STALE_IP="${STALE_IP:-203.0.113.10}"   # TEST-NET-3, never routable

# --approot is explicit: Windows builds of stackql default it to the cwd.
STACKQL=(stackql --approot "${STACKQL_APPROOT:-$HOME/.stackql}")
q() { "${STACKQL[@]}" exec -o csv -H "$1"; }
run() { printf '\n$ %s\n' "$*" >&2; "$@"; }

what=("$@"); [ ${#what[@]} -eq 0 ] && what=(tag ssh edge)

echo "# locating the service by tag service=$DEMO_HOST in $AWS_REGION" >&2
INSTANCE_ID=""
for id in $(q "SELECT resource_id FROM aws.ec2.tags WHERE region = '$AWS_REGION' AND resource_type = 'instance' AND key = 'service' AND value = '$DEMO_HOST'"); do
  state="$(q "SELECT JSON_EXTRACT(state, '\$.name') FROM aws.ec2.instances WHERE region = '$AWS_REGION' AND instance_id = '$id'" | head -1)"
  [ "$state" = "running" ] && INSTANCE_ID="$id" && break
done
[ -n "$INSTANCE_ID" ] || { echo "no running instance tagged service=$DEMO_HOST; run stackql-deploy build first" >&2; exit 1; }
SG_ID="$(q "SELECT resource_id FROM aws.ec2.tags WHERE region = '$AWS_REGION' AND resource_type = 'security-group' AND key = 'service' AND value = '$DEMO_HOST'" | head -1)"
RECORD_ID="$(q "SELECT id FROM cloudflare.dns.zones_dns_records WHERE zone_id = '$CLOUDFLARE_ZONE_ID' AND name = '$DEMO_HOST.$DEMO_DOMAIN' AND type = 'A'" | head -1)"
echo "# instance $INSTANCE_ID, security group $SG_ID, record $RECORD_ID" >&2

for d in "${what[@]}"; do
  case "$d" in
    tag)
      run "${STACKQL[@]}" exec \
        "UPDATE awscc.ec2.instances SET PatchDocument = string('[{\"op\":\"replace\",\"path\":\"/Tags\",\"value\":[{\"Key\":\"Name\",\"Value\":\"$DEMO_HOST-dev\"},{\"Key\":\"service\",\"Value\":\"$DEMO_HOST\"},{\"Key\":\"cost-centre\",\"Value\":\"demo\"},{\"Key\":\"stackql:stack-name\",\"Value\":\"service-footprint\"},{\"Key\":\"stackql:stack-env\",\"Value\":\"dev\"}]}]') WHERE Identifier = '$INSTANCE_ID' AND region = '$AWS_REGION'" ;;
    ssh)
      run "${STACKQL[@]}" exec \
        "INSERT INTO awscc.ec2.security_group_ingresses (GroupId, IpProtocol, FromPort, ToPort, CidrIp, Description, region) SELECT '$SG_ID', 'tcp', 22, 22, '0.0.0.0/0', 'temporary debug access', '$AWS_REGION'" ;;
    edge)
      run "${STACKQL[@]}" exec \
        "DELETE FROM cloudflare.dns.records WHERE zone_id = '$CLOUDFLARE_ZONE_ID' AND dns_record_id = '$RECORD_ID'" ;;
    dangling)
      run "${STACKQL[@]}" exec \
        "INSERT INTO cloudflare.dns.zones_dns_records (zone_id, name, type, content, ttl, proxied, comment) SELECT '$CLOUDFLARE_ZONE_ID', '$DEMO_HOST-old.$DEMO_DOMAIN', 'A', '$STALE_IP', 300, false, 'left behind'" ;;
    *) echo "unknown drift kind: $d" >&2; exit 2 ;;
  esac
done

echo
echo "drift introduced (${what[*]}) on $DEMO_HOST in $AWS_REGION and $DEMO_DOMAIN."
