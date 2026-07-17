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
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import {
  HOST_V2_RUNTIME_SOURCE_HASHES,
  HOST_V2_SOURCE_HASHES,
  LEGACY_PUBLIC_DESCRIPTOR_HASHES,
  projectPrivateHostV2Descriptor,
  type FrozenCommonsMetadata,
  type FrozenCommonsNetworkBinding,
  type FrozenHostV2Errors,
  type FrozenHostV2Operations,
  type FrozenPapiManifest,
} from "../src/internal/host-v2-descriptor.ts";
import { COMMONS_NETWORK_BINDING } from "../src/network-binding.ts";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "../../../..");
const sources = {
  cddl: "docs/specs/origin-host-registry-v2.cddl",
  operations: "docs/specs/origin-host-registry-v2.operations.json",
  errors: "docs/specs/origin-host-registry-v2.errors.json",
  schema: "docs/specs/origin-host-registry-v2.schema.json",
} as const;
const runtimeSources = {
  metadataIdentity: "docs/sdk/metadata/commons-v29.json",
  metadataScale: "product-sdk/packages/descriptors/.papi/metadata/commons.scale",
  papiManifest: "product-sdk/packages/descriptors/generated/commons-papi-manifest.json",
} as const;
const legacy = {
  packageIndex: "product-sdk/packages/origin-sdk-descriptors/src/index.ts",
  packageManifest: "product-sdk/packages/origin-sdk-descriptors/package.json",
  runtimeDescriptor: "product-sdk/packages/descriptors/generated/orbis-descriptor.json",
  networkBinding: "product-sdk/packages/origin-sdk-descriptors/src/network-binding.ts",
} as const;
const sha256 = (path: string): string => createHash("sha256")
  .update(readFileSync(resolve(repoRoot, path)))
  .digest("hex");

for (const key of Object.keys(sources) as (keyof typeof sources)[]) {
  const actual = sha256(sources[key]);
  if (actual !== HOST_V2_SOURCE_HASHES[key]) throw new Error(`frozen host v2 ${key} hash drift: ${actual}`);
}
for (const key of Object.keys(runtimeSources) as (keyof typeof runtimeSources)[]) {
  const actual = sha256(runtimeSources[key]);
  if (actual !== HOST_V2_RUNTIME_SOURCE_HASHES[key]) {
    throw new Error(`frozen host v2 runtime ${key} hash drift: ${actual}`);
  }
}
for (const key of Object.keys(legacy) as (keyof typeof legacy)[]) {
  const actual = sha256(legacy[key]);
  if (actual !== LEGACY_PUBLIC_DESCRIPTOR_HASHES[key]) throw new Error(`legacy public descriptor ${key} drift: ${actual}`);
}

const operations = JSON.parse(readFileSync(resolve(repoRoot, sources.operations), "utf8")) as FrozenHostV2Operations;
const errors = JSON.parse(readFileSync(resolve(repoRoot, sources.errors), "utf8")) as FrozenHostV2Errors;
const metadata = JSON.parse(
  readFileSync(resolve(repoRoot, runtimeSources.metadataIdentity), "utf8"),
) as FrozenCommonsMetadata;
const papi = JSON.parse(
  readFileSync(resolve(repoRoot, runtimeSources.papiManifest), "utf8"),
) as FrozenPapiManifest;
const descriptor = projectPrivateHostV2Descriptor(
  operations,
  errors,
  metadata,
  papi,
  COMMONS_NETWORK_BINDING as FrozenCommonsNetworkBinding,
);
const serialized = `${JSON.stringify(descriptor, null, 2)}\n`;
const output = resolve(repoRoot, "product-sdk/packages/origin-sdk-descriptors/src/internal/generated/host-v2-descriptor.json");
if (process.argv.includes("--check")) {
  if (readFileSync(output, "utf8") !== serialized) {
    throw new Error("private host v2 descriptor drift; run generate-host-v2-descriptor.ts");
  }
  process.stdout.write(`PASS private host v2 descriptor: ${descriptor.operationCount} operations / ${descriptor.errorCount} errors\n`);
} else {
  mkdirSync(dirname(output), { recursive: true });
  writeFileSync(output, serialized);
  process.stdout.write(`${output}\n`);
}
