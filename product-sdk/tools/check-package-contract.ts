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

import { existsSync, readFileSync, readdirSync, statSync } from "node:fs";
import { resolve } from "node:path";

const packagesRoot = resolve(import.meta.dirname, "../packages");
const manifests = readdirSync(packagesRoot, { withFileTypes: true })
  .filter((entry) => entry.isDirectory() && entry.name.startsWith("origin-sdk-"))
  .map((entry) => resolve(packagesRoot, entry.name, "package.json"))
  .filter(existsSync)
  .sort();

const fail = (message: string): never => { throw new Error(`package contract: ${message}`); };
for (const path of manifests) {
  const manifest = JSON.parse(readFileSync(path, "utf8"));
  if (!/^@cord-network\/origin-sdk(?:-[a-z0-9-]+)?$/.test(manifest.name))
    fail(`${path} has invalid package name`);
  if (manifest.private === true) fail(`${manifest.name} must be publishable`);
  if (manifest.type !== "module" || manifest.sideEffects !== false)
    fail(`${manifest.name} must be side-effect-free ESM`);
  if (JSON.stringify(manifest.files) !== JSON.stringify(["dist", "README.md"]))
    fail(`${manifest.name} must publish only dist and README.md`);
  if (!manifest.exports || !manifest.exports["."])
    fail(`${manifest.name} has no root export`);
  const exportTargets: string[] = [];
  const collectTargets = (value: unknown): void => {
    if (typeof value === "string") exportTargets.push(value);
    else if (typeof value === "object" && value !== null)
      for (const nested of Object.values(value)) collectTargets(nested);
  };
  collectTargets(manifest.exports);
  if (exportTargets.some((target) => target.includes("src/")
    || (target.endsWith(".ts") && !target.endsWith(".d.ts"))))
    fail(`${manifest.name} exports source TypeScript`);
  if (!manifest.scripts?.build || !manifest.scripts?.check)
    fail(`${manifest.name} lacks build/check scripts`);
  if (!existsSync(resolve(path, "../README.md"))) fail(`${manifest.name} lacks a README`);
  const dist = resolve(path, "../dist");
  if (!existsSync(dist)) fail(`${manifest.name} has not been built`);
  const bytes = (directory: string): number => readdirSync(directory, { withFileTypes: true })
    .reduce((total, entry) => total + (entry.isDirectory()
      ? bytes(resolve(directory, entry.name))
      : statSync(resolve(directory, entry.name)).size), 0);
  const budget = manifest.cord?.bundleBudgetBytes;
  if (typeof budget === "number" && bytes(dist) > budget)
    fail(`${manifest.name} exceeds its ${budget}-byte distribution budget`);
}

process.stdout.write(`PASS package distribution contract: packages=${manifests.length}\n`);
