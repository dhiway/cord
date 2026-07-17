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

import { createHash } from "node:crypto";
import { existsSync, readFileSync, readdirSync, rmSync, statSync, writeFileSync } from "node:fs";
import { dirname, relative, resolve } from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const sdkRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const repoRoot = resolve(sdkRoot, "..");
const chainRoot = resolve(sdkRoot, "packages/descriptors/chains/commons");
const generatedRoot = resolve(chainRoot, "generated");
const config = resolve(chainRoot, ".papi/polkadot-api.json");
const metadata = resolve(sdkRoot, "packages/descriptors/.papi/metadata/commons.scale");
const metadataIdentity = JSON.parse(
  readFileSync(resolve(repoRoot, "docs/sdk/metadata/commons-v29.json"), "utf8"),
);
const currentRuntime = JSON.parse(
  readFileSync(resolve(repoRoot, "origin/orbis/runtime/vectors/transaction-policy-v8/metadata-hash.json"), "utf8"),
);
const output = resolve(sdkRoot, "packages/descriptors/generated/commons-papi-manifest.json");
const sourceHeader = readFileSync(resolve(repoRoot, "HEADER-GPL3"), "utf8");
const sha256 = (bytes: Uint8Array | string) => createHash("sha256").update(bytes).digest("hex");

if (sha256(readFileSync(metadata)) !== metadataIdentity.scale_sha256)
  throw new Error("checked-in Commons metadata digest drift");
if (statSync(metadata).size !== metadataIdentity.scale_bytes)
  throw new Error("checked-in Commons metadata size drift");

rmSync(generatedRoot, { recursive: true, force: true });
const papi = resolve(sdkRoot, "node_modules/.bin/papi");
const generated = spawnSync(papi, ["generate", "--config", config], {
  cwd: chainRoot,
  encoding: "utf8",
  stdio: "pipe",
});
if (generated.status !== 0) {
  process.stderr.write(generated.stdout ?? "");
  process.stderr.write(generated.stderr ?? "");
  throw new Error(`PAPI descriptor generation failed with status ${generated.status}`);
}

const files = (directory: string): string[] => readdirSync(directory, { withFileTypes: true })
  .flatMap((entry) => entry.isDirectory()
    ? files(resolve(directory, entry.name))
    : [resolve(directory, entry.name)])
  .sort();
for (const path of files(generatedRoot)) {
  if (!path.endsWith(".js") && !path.endsWith(".d.ts")) continue;
  const body = readFileSync(path, "utf8");
  writeFileSync(path, body.startsWith(sourceHeader) ? body : `${sourceHeader}\n${body}`);
}

const distributable = files(generatedRoot)
  .filter((path) => !path.endsWith("/.gitignore"))
  .map((path) => {
    const bytes = readFileSync(path);
    return {
      path: relative(generatedRoot, path).replaceAll("\\", "/"),
      bytes: bytes.length,
      sha256: sha256(bytes),
    };
  });
const manifest = {
  schema: "cord.commons-papi-descriptor.v1",
  generator: { package: "polkadot-api", version: "2.1.6" },
  metadata: {
    path: "product-sdk/packages/descriptors/.papi/metadata/commons.scale",
    scale_bytes: metadataIdentity.scale_bytes,
    scale_sha256: metadataIdentity.scale_sha256,
    runtime_metadata_version: metadataIdentity.runtime_metadata_version,
    runtime_rfc78_hash: metadataIdentity.runtime_rfc78_hash,
  },
  entry: "commons",
  availability: {
    runtime_metadata_current: metadataIdentity.spec_version === currentRuntime.spec_version,
    sdk_admission: metadataIdentity.spec_version === currentRuntime.spec_version,
    reason: metadataIdentity.spec_version === currentRuntime.spec_version
      ? null
      : "checked-in PAPI metadata predates the current Commons runtime",
  },
  output: distributable,
};
const serialized = `${JSON.stringify(manifest, null, 2)}\n`;
if (process.argv.includes("--check")) {
  if (!existsSync(output) || readFileSync(output, "utf8") !== serialized)
    throw new Error("Commons PAPI descriptor drift; run npm run generate:papi");
  process.stdout.write(`PASS Commons PAPI descriptor: ${distributable.length} generated files\n`);
} else {
  writeFileSync(output, serialized);
  process.stdout.write(`${output}\n`);
}
