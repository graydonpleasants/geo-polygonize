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

import { z } from "zod";

export const RootCauseSchema = z.object({
  id: z.string(),
  title: z.string(),
  severity: z.enum(["critical", "high", "medium", "low"]),
  issues: z.array(z.number()),
  files: z.array(z.string()),
  description: z.string(),
  solution_summary: z.string(),
});

export const TaskSchema = z.object({
  id: z.string(),
  title: z.string(),
  root_cause: z.string(),
  issues: z.array(z.number()),
  files: z.array(z.string()),
  new_files: z.array(z.string()),
  test_files: z.array(z.string()),
  risk: z.enum(["low", "medium", "high"]),
  target_branch: z.string().optional(),
  prompt: z.string(),
});

export const UnaddressableIssueSchema = z.object({
  issue: z.number(),
  reason: z.string(),
  suggested_owner: z.string(),
});

export const IssueAnalysisSchema = z.object({
  repo: z.string(),
  analyzed_at: z.string(),
  root_causes: z.array(RootCauseSchema),
  tasks: z.array(TaskSchema),
  unaddressable: z.array(UnaddressableIssueSchema),
  file_ownership: z.record(z.string(), z.string()),
});

export type IssueAnalysis = z.infer<typeof IssueAnalysisSchema>;
export type Task = z.infer<typeof TaskSchema>;
