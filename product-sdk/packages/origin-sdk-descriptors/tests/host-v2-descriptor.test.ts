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

import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import { spawnSync } from "node:child_process";
import test from "node:test";
import {
  HOST_V2_OPERATION_SIGNATURES,
  HOST_V2_PROTOCOL,
  HOST_V2_REQUIRED_NEGOTIATION_FEATURES,
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
import {
  HOST_V2_FEATURE_IDS as GENERATED_HOST_V2_FEATURE_IDS,
  HOST_V2_JSON_PROJECTION_SHA256,
  HOST_V2_PROTOCOL as GENERATED_HOST_V2_PROTOCOL,
  HOST_V2_REGISTRY_SHA256,
} from "../../origin-sdk-host/src/internal/v2/generated.ts";

const repoRoot = resolve(import.meta.dirname, "../../../..");
const json = async <Value>(path: string): Promise<Value> => JSON.parse(
  await readFile(resolve(repoRoot, path), "utf8"),
) as Value;
const sha256 = async (path: string): Promise<string> => createHash("sha256")
  .update(await readFile(resolve(repoRoot, path)))
  .digest("hex");

async function frozenInputs() {
  return {
    operations: await json<FrozenHostV2Operations>("docs/specs/origin-host-registry-v2.operations.json"),
    errors: await json<FrozenHostV2Errors>("docs/specs/origin-host-registry-v2.errors.json"),
    metadata: await json<FrozenCommonsMetadata>("docs/sdk/metadata/commons-v29.json"),
    papi: await json<FrozenPapiManifest>("product-sdk/packages/descriptors/generated/commons-papi-manifest.json"),
    network: COMMONS_NETWORK_BINDING as FrozenCommonsNetworkBinding,
  };
}

test("private descriptor projects exact storage, separate Identity, and signing surfaces", async () => {
  const { operations, errors, metadata, papi, network } = await frozenInputs();
  const projected = projectPrivateHostV2Descriptor(operations, errors, metadata, papi, network);
  const generated = await json<typeof projected>(
    "product-sdk/packages/origin-sdk-descriptors/src/internal/generated/host-v2-descriptor.json",
  );
  assert.deepEqual(generated, projected);
  assert.equal(projected.protocol.id, HOST_V2_PROTOCOL);
  assert.equal(projected.operationCount, 34);
  assert.equal(projected.surfaces.storage.length, 26);
  assert.equal(projected.surfaces.identity.composite, false);
  assert.deepEqual(projected.surfaces.identity.operations.map(({ code }) => code), [1100, 1101, 1102, 1103, 1104, 1105, 1106]);
  assert.deepEqual(projected.surfaces.identity.operations.map(({ name }) => name), [
    "identity.account", "identity.profile.read", "identity.profile.disclose",
    "identity.humanity.status", "identity.humanity.prove", "identity.subject.derive",
    "identity.entitlements.read",
  ]);
  assert.equal(projected.surfaces.transaction.separateSigning, true);
  assert.deepEqual(projected.surfaces.transaction.operations.map(({ code, name }) => ({ code, name })), [
    { code: 1200, name: "transaction.sign" },
  ]);
  assert.equal(HOST_V2_OPERATION_SIGNATURES.length, 34);
  assert.deepEqual(projected.sources, HOST_V2_SOURCE_HASHES);
  assert.deepEqual(projected.requiredNegotiationFeatures, HOST_V2_REQUIRED_NEGOTIATION_FEATURES);
  assert.deepEqual(Object.keys(projected.features).sort(), [...HOST_V2_REQUIRED_NEGOTIATION_FEATURES]);
  assert.equal(GENERATED_HOST_V2_PROTOCOL, HOST_V2_PROTOCOL);
  assert.equal(HOST_V2_REGISTRY_SHA256, HOST_V2_SOURCE_HASHES.cddl);
  assert.equal(HOST_V2_JSON_PROJECTION_SHA256, HOST_V2_SOURCE_HASHES.schema);
  assert.deepEqual(GENERATED_HOST_V2_FEATURE_IDS, HOST_V2_REQUIRED_NEGOTIATION_FEATURES);
});

test("private descriptor is bound to generated Commons runtime metadata", async () => {
  const { operations, errors, metadata, papi, network } = await frozenInputs();
  const { runtimeBinding, runtimeSources } = projectPrivateHostV2Descriptor(
    operations,
    errors,
    metadata,
    papi,
    network,
  );
  assert.deepEqual(runtimeSources, HOST_V2_RUNTIME_SOURCE_HASHES);
  assert.deepEqual(runtimeBinding, {
    runtime: "commons",
    specName: "commons",
    genesisHash: COMMONS_NETWORK_BINDING.genesis_hash,
    specVersion: COMMONS_NETWORK_BINDING.spec_version,
    transactionVersion: COMMONS_NETWORK_BINDING.transaction_version,
    metadataVersion: 14,
    metadataRfc78Hash: COMMONS_NETWORK_BINDING.metadata_hash,
    metadataScale: {
      path: "product-sdk/packages/descriptors/.papi/metadata/commons.scale",
      bytes: 576823,
      sha256: HOST_V2_RUNTIME_SOURCE_HASHES.metadataScale,
    },
    generatedDescriptor: {
      schema: "cord.commons-papi-descriptor.v1",
      entry: "commons",
      generator: "polkadot-api@2.1.6",
      manifestSha256: HOST_V2_RUNTIME_SOURCE_HASHES.papiManifest,
    },
    network: {
      descriptorContractSha256: COMMONS_NETWORK_BINDING.descriptor_contract_sha256,
      chainSpecSourceSha256: COMMONS_NETWORK_BINDING.chain_spec_source_sha256,
      activationState: COMMONS_NETWORK_BINDING.activation_state,
      productionActivationReady: false,
    },
  });
});

test("deterministic generator rejects checked-in descriptor drift", () => {
  const tool = resolve(repoRoot, "product-sdk/packages/origin-sdk-descriptors/tools/generate-host-v2-descriptor.ts");
  const checked = spawnSync(process.execPath, ["--experimental-strip-types", tool, "--check"], {
    cwd: repoRoot,
    encoding: "utf8",
  });
  assert.equal(checked.status, 0, checked.stderr);
	assert.match(checked.stdout, /PASS private host v2 descriptor: 34 operations \/ 91 errors/);
});

test("projection fails closed on operation and error drift", async () => {
  const { operations, errors, metadata, papi, network } = await frozenInputs();
  const changed = structuredClone(operations) as { operations: Array<FrozenHostV2Operations["operations"][number]> } & Omit<FrozenHostV2Operations, "operations">;
  changed.operations[4] = { ...changed.operations[4]!, feature_id: "storage.legacy" };
  assert.throws(() => projectPrivateHostV2Descriptor(changed, errors, metadata, papi, network), /signature drift/);
  assert.throws(() => projectPrivateHostV2Descriptor(
    { ...operations, operations: operations.operations.slice(0, -1) },
    errors,
    metadata,
    papi,
    network,
  ), /operation count drift/);
  assert.throws(() => projectPrivateHostV2Descriptor(operations, {
    ...errors,
    errors: errors.errors.map((error, index) => index === 1 ? { ...error, code: errors.errors[0]!.code } : error),
  }, metadata, papi, network), /duplicate host v2 error code/);
  assert.throws(() => projectPrivateHostV2Descriptor(
    operations,
    errors,
    metadata,
    { ...papi, metadata: { ...papi.metadata, scale_bytes: papi.metadata.scale_bytes + 1 } },
    network,
  ), /generated PAPI metadata scale_bytes drift/);
  assert.throws(() => projectPrivateHostV2Descriptor(
    operations,
    errors,
    metadata,
    papi,
    { ...network, metadata_hash: `0x${"0".repeat(64)}` },
  ), /network and generated metadata binding drift/);
});

test("host v2 descriptor stays non-public and legacy descriptor bytes stay unchanged", async () => {
  const legacy = {
    packageIndex: "product-sdk/packages/origin-sdk-descriptors/src/index.ts",
    packageManifest: "product-sdk/packages/origin-sdk-descriptors/package.json",
    runtimeDescriptor: "product-sdk/packages/descriptors/generated/orbis-descriptor.json",
    networkBinding: "product-sdk/packages/origin-sdk-descriptors/src/network-binding.ts",
  } as const;
  for (const key of Object.keys(legacy) as (keyof typeof legacy)[]) {
    assert.equal(await sha256(legacy[key]), LEGACY_PUBLIC_DESCRIPTOR_HASHES[key], key);
  }
  const publicIndex = await readFile(resolve(repoRoot, legacy.packageIndex), "utf8");
  const packageManifest = await json<{ exports: Record<string, unknown> }>(legacy.packageManifest);
  assert.doesNotMatch(publicIndex, /HOST_V2|host-v2-descriptor|cord\.origin\.host\/2/);
  assert.deepEqual(Object.keys(packageManifest.exports), [".", "./commons"]);
  assert.equal(JSON.stringify(packageManifest).includes("host-v2"), false);
  const legacyDescriptor = await json<{ contractVersion: number; nativeHostContract: { version: number } }>(legacy.runtimeDescriptor);
  assert.equal(legacyDescriptor.contractVersion, 1);
  assert.equal(legacyDescriptor.nativeHostContract.version, 1);
});
