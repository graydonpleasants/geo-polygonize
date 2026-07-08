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

import path from "node:path";
import { fileURLToPath } from "node:url";
import { execFileSync } from "node:child_process";

/**
 * Validates the FLEET_ID to prevent path traversal and other injection attacks.
 * Only alphanumeric characters and underscores are allowed.
 */
export function validateFleetId(id: string): string {
  if (!/^[a-zA-Z0-9_]+$/.test(id)) {
    throw new Error(`Invalid FLEET_ID: "${id}". Only alphanumeric characters and underscores are allowed.`);
  }
  return id;
}

const rawFleetId = process.env.FLEET_ID || new Intl.DateTimeFormat("en-CA", {
  year: "numeric",
  month: "2-digit",
  day: "2-digit"
}).format(new Date()).replaceAll("-", "_");

// Use FLEET_ID environment variable if provided, otherwise generate default date-based ID
export const FLEET_ID = validateFleetId(rawFleetId);

let ROOT_DIR_VAL: string;
const __dirname = path.dirname(fileURLToPath(import.meta.url));

try {
  ROOT_DIR_VAL = execFileSync("git", ["rev-parse", "--show-toplevel"], {
    cwd: __dirname,
    encoding: "utf8",
  }).trim();
} catch {
  ROOT_DIR_VAL = path.resolve(__dirname, "../..");
}

export const ROOT_DIR = ROOT_DIR_VAL;

const BASE_FLEET_DIR = path.resolve(ROOT_DIR, ".fleet");
export const FLEET_DIR = path.join(BASE_FLEET_DIR, FLEET_ID);

// Verify that the resolved FLEET_DIR is within the expected bounds
const resolvedFleetDir = path.resolve(FLEET_DIR);
if (!resolvedFleetDir.startsWith(BASE_FLEET_DIR)) {
  throw new Error(`Security breach: FLEET_DIR ("${resolvedFleetDir}") is outside of ${BASE_FLEET_DIR}`);
}
