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
import { findUpSync } from "find-up";

// Use FLEET_ID environment variable if provided, otherwise generate default date-based ID
export const FLEET_ID = process.env.FLEET_ID || new Intl.DateTimeFormat("en-CA", {
  year: "numeric",
  month: "2-digit",
  day: "2-digit"
}).format(new Date()).replaceAll("-", "_");

export const ROOT_DIR = path.dirname(findUpSync(".git")!);
export const FLEET_DIR = path.join(ROOT_DIR, ".fleet", FLEET_ID);
