// config for cross-provider.iql
// external vars supplied with --var from .env:
//   --var zone=$DEMO_DOMAIN --var region=$AWS_REGION
{
  zone: std.extVar('zone'),
  region: std.extVar('region'),
}
