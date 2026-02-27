// Copyright 2026 Google LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

import { Octokit } from "octokit";
import { cachePlugin } from "./cache-plugin.js";
import { getGitRepoInfo } from "./git.js";

/** Octokit with built-in ETag caching */
export const CachedOctokit = Octokit.plugin(cachePlugin) as typeof Octokit;

export interface LinkedPR {
  number: number;
  title: string;
  headRef: string;
  html_url: string;
}

export interface EnhancedIssue {
  number: number;
  title: string;
  html_url: string;
  user: { login: string } | null;
  author_association: string;
  state: string;
  state_reason?: string | null;
  locked: boolean;
  active_lock_reason?: string | null;
  comments: number;
  created_at: string;
  updated_at: string;
  closed_at?: string | null;
  closed_by?: { login: string } | null;
  labels: ({ name?: string } | string)[];
  assignees?: { login: string }[] | null;
  milestone?: { title: string } | null;
  draft?: boolean;
  pull_request?: any;
  reactions?: any;
  body?: string | null;
  linkedPrs?: LinkedPR[];
}

/** Fetch open issues from the current repository */
export async function getIssues(
  options?: { perPage?: number; state?: "open" | "closed" | "all" }
) {
  const repoInfo = await getGitRepoInfo();
  const octokit = new CachedOctokit({
    auth: process.env.GITHUB_TOKEN,
  });

  // 1. Fetch issues
  const { data: issues } = await octokit.rest.issues.listForRepo({
    owner: repoInfo.owner,
    repo: repoInfo.repo,
    state: options?.state ?? "open",
    per_page: options?.perPage ?? 30,
  });

  const enhancedIssues: EnhancedIssue[] = issues.filter((issue) => !issue.pull_request) as EnhancedIssue[];

  // 2. Fetch open PRs
  const { data: pulls } = await octokit.rest.pulls.list({
    owner: repoInfo.owner,
    repo: repoInfo.repo,
    state: "open",
    per_page: 100, // Fetch up to 100 open PRs
  });

  // 3. Link PRs to issues
  // Strategy: naive text search in PR body for "Fixes #123", "Closes #123", or just "#123" if context implies.
  // GitHub API has a `timeline` endpoint for issues that shows cross-referenced events, but that's expensive (N+1).
  // We'll parse PR bodies for "Fixes #N", "Closes #N", "Resolves #N" etc.

  const issuePrMap = new Map<number, LinkedPR[]>();
  const closeKeywords = /((?:close|closes|closed|fix|fixes|fixed|resolve|resolves|resolved)\s+)(#)(\d+)/gi;

  for (const pr of pulls) {
    if (!pr.body) continue;

    // Find all matches
    const matches = [...pr.body.matchAll(closeKeywords)];
    for (const match of matches) {
      const issueNum = parseInt(match[3], 10);
      if (!issuePrMap.has(issueNum)) {
        issuePrMap.set(issueNum, []);
      }
      issuePrMap.get(issueNum)!.push({
        number: pr.number,
        title: pr.title,
        headRef: pr.head.ref,
        html_url: pr.html_url
      });
    }
  }

  // Associate found PRs with issues
  for (const issue of enhancedIssues) {
    if (issuePrMap.has(issue.number)) {
      issue.linkedPrs = issuePrMap.get(issue.number);
    }
  }

  return enhancedIssues;
}

export { cachePlugin } from "./cache-plugin.js";
