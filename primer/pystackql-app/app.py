"""GitHub org estate report using pystackql.

Act 1, slide "How to use StackQL" (Application & Data Integration row).
pip install pystackql pandas
Prereqs: none (public GitHub data). STACKQL_GITHUB_USERNAME and
STACKQL_GITHUB_PASSWORD in the environment lift the rate limit.
pystackql downloads its own stackql binary on first run; app_root points it at
the provider cache the CLI uses.
"""

import os

import pandas as pd
from pystackql import StackQL

stackql = StackQL(output="pandas", app_root=os.path.expanduser("~/.stackql"))

# estate inventory
repos = stackql.execute("""
SELECT name, language, stargazers_count AS stars, forks_count AS forks, archived
FROM github.repos.repos
WHERE org = 'stackql'
""")
# every column arrives as text; make the counters numeric before aggregating
repos[["stars", "forks"]] = repos[["stars", "forks"]].apply(pd.to_numeric)
active = repos[repos["archived"] == "false"]

print("stackql org: active repos by stars\n")
top = active.sort_values("stars", ascending=False).head(5)
print(top[["name", "language", "stars", "forks"]].to_string(index=False))

# stars by language, aggregated in pandas
by_language = active.groupby("language")["stars"].sum().sort_values(ascending=False)
print("\nstars by language\n")
print(by_language.head(5).to_string())

# live operational signal: the latest CI runs
runs = stackql.execute("""
SELECT name, head_branch AS branch, conclusion, substr(created_at, 1, 10) AS day
FROM (
  SELECT name, head_branch, conclusion, created_at
  FROM github.actions.workflow_runs
  WHERE owner = 'stackql' AND repo = 'stackql'
  ORDER BY created_at DESC
) t
LIMIT 5
""")
print("\nlatest CI runs on stackql/stackql\n")
print(runs[["name", "branch", "conclusion", "day"]].to_string(index=False))

# writes use executeStmt (no result set); needs a token with write access
# stackql.executeStmt("""
# REPLACE github.repos.topics
# SET names = '["stackql","rust","mcp","golden-path"]'
# WHERE owner = 'stackql' AND repo = 'rust-embedded-mcp-with-stackql'
# """)
