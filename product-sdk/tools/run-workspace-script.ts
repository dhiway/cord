// This file is part of CORD – https://cord.network

// Copyright (C) Dhiway Networks Pvt. Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later

// CORD is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// CORD is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with CORD. If not, see <https://www.gnu.org/licenses/>.

import { existsSync, readFileSync, readdirSync } from "node:fs";
import { resolve } from "node:path";
import { spawnSync } from "node:child_process";

const script = process.argv[2];
if (!script) throw new Error("workspace script name is required");
const packages = resolve(import.meta.dirname, "../packages");
const discovered = readdirSync(packages, { withFileTypes: true })
  .filter((entry) => entry.isDirectory() && entry.name.startsWith("origin-sdk-"))
  .map((entry) => resolve(packages, entry.name))
  .filter((path) => existsSync(resolve(path, "package.json")))
  .sort();
const manifests = new Map(discovered.map((path) => {
  const manifest = JSON.parse(readFileSync(resolve(path, "package.json"), "utf8"));
  return [manifest.name as string, { path, manifest }] as const;
}));
const workspaces: string[] = [];
const visiting = new Set<string>();
const visited = new Set<string>();
const visit = (name: string): void => {
  if (visited.has(name)) return;
  if (visiting.has(name)) throw new Error(`workspace dependency cycle at ${name}`);
  visiting.add(name);
  const workspace = manifests.get(name);
  if (!workspace) throw new Error(`unknown workspace ${name}`);
  const dependencies = { ...workspace.manifest.dependencies, ...workspace.manifest.devDependencies };
  for (const dependency of Object.keys(dependencies).sort()) {
    if (manifests.has(dependency)) visit(dependency);
  }
  visiting.delete(name);
  visited.add(name);
  workspaces.push(workspace.path);
};
for (const name of [...manifests.keys()].sort()) visit(name);

for (const workspace of workspaces) {
  const result = spawnSync("npm", ["run", script, "--if-present"], {
    cwd: workspace,
    encoding: "utf8",
    stdio: "inherit",
  });
  if (result.status !== 0) process.exit(result.status ?? 1);
}
process.stdout.write(`PASS workspace ${script}: packages=${workspaces.length}\n`);
