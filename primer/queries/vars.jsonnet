// config for branch-protection.iql
// external var 'org' supplied with --var, e.g. --var org=stackql
local org = std.extVar('org');

// the repos whose default branch must be protected, rendered as a SQL IN list
// (the JOIN fans out to one API call per repo, so keep it short)
local key_repos = ['stackql', 'stackql-deploy', 'pystackql'];

{
  org: org,
  key_repos: "('" + std.join("', '", key_repos) + "')",
}
