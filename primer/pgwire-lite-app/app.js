// Node app talking to a stackql server over the postgres wire protocol.
// Act 1, slide "How to use StackQL" (Application & Data Integration row).
//
// start the server first:  stackql srv --pgsrv.port 5466
// then:                    npm install && npm start

import { runQuery } from '@stackql/pgwire-lite';

const connectionOptions = {
  user: 'stackql',
  database: 'stackql',
  host: 'localhost',
  port: 5466,
  debug: false,
};

// estate inventory: the most starred active repos in the org
const reposSql = `
SELECT name, stars, forks
FROM (
  SELECT name, stargazers_count AS stars, forks_count AS forks
  FROM github.repos.repos
  WHERE org = 'stackql' AND archived = 0
  ORDER BY stargazers_count DESC
) t
LIMIT 5`;

// live operational signal: the latest CI runs
const runsSql = `
SELECT name, head_branch AS branch, conclusion, substr(created_at, 1, 10) AS day
FROM (
  SELECT name, head_branch, conclusion, created_at
  FROM github.actions.workflow_runs
  WHERE owner = 'stackql' AND repo = 'stackql'
  ORDER BY created_at DESC
) t
LIMIT 5`;

const repos = await runQuery(connectionOptions, reposSql);
console.log('stackql org: active repos by stars');
console.table(repos.data);

const runs = await runQuery(connectionOptions, runsSql);
console.log('latest CI runs on stackql/stackql:');
console.table(runs.data);
