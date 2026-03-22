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

import { jules } from '@google/jules-sdk'
import { getGitRepoInfo, getCurrentBranch } from './github/git.js'
import { readFileSync } from 'node:fs'
import path from 'node:path'
import { ROOT_DIR } from './config.js'

const repoInfo = await getGitRepoInfo()
const baseBranch = process.env.FLEET_BASE_BRANCH ?? await getCurrentBranch()

console.log(`🔍 Starting Roadmap Analyzer session for ${repoInfo.fullName} (branch: ${baseBranch})`)

const roadmapPath = path.join(ROOT_DIR, 'ROADMAP.md')
const roadmapText = readFileSync(roadmapPath, 'utf8')

const prompt = `Analyze the ROADMAP.md to enhance our roadmap with new tasks to push geo-polygonize to be the state of the art library it should be.

## Current ROADMAP.md
\`\`\`markdown
${roadmapText}
\`\`\`
`

const session = await jules.session({
  prompt,
  source: {
    github: repoInfo.fullName,
    baseBranch,
  },
  automationMode: 'AUTO_CREATE_PR',
  requirePlanApproval: false
})

console.log(`✅ Roadmap Analyzer session started: ${session.id}`)
