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
import test from "node:test";
import { createCommonsChainClient } from "@cord-network/origin-sdk-chain-client";
import { parseContentCid, rawContentAddress } from "@cord-network/origin-sdk-cloud-storage";
import { COMMONS_NETWORK_BINDING } from "@cord-network/origin-sdk-descriptors";
import { contentCommitment, nameId, type NamesRuntimeAdapter } from "@cord-network/origin-sdk-names";
import type { PreparedTransaction } from "@cord-network/origin-sdk-tx";
import {
  ORIGIN_APP_MANIFEST_SCHEMA,
  createOriginAppsClient,
  decodeOriginAppManifest,
  encodeOriginAppManifest,
  type OriginAppContentStore,
  type OriginAppManifestV1,
} from "../src/index.ts";

const at = `0x${"11".repeat(32)}` as const;
const appName = nameId(`0x${"22".repeat(32)}`);
const owner = "5Owner" as never;
const tx: PreparedTransaction = { async *signSubmitAndWatch() {} };
const binding = {
  genesis_hash: COMMONS_NETWORK_BINDING.genesis_hash,
  spec_version: COMMONS_NETWORK_BINDING.spec_version,
  transaction_version: COMMONS_NETWORK_BINDING.transaction_version,
  metadata_hash: COMMONS_NETWORK_BINDING.metadata_hash,
  descriptor_contract_sha256: COMMONS_NETWORK_BINDING.descriptor_contract_sha256,
  chain_spec_source_sha256: COMMONS_NETWORK_BINDING.chain_spec_source_sha256,
};

const bundleBytes = new TextEncoder().encode("bundle");
const manifest: OriginAppManifestV1 = {
  schema: ORIGIN_APP_MANIFEST_SCHEMA,
  schemaVersion: 1,
  product: { id: "festival.app", name: "Festival" },
  nameId: appName,
  version: "1.2.3",
  channel: "stable",
  entrypoint: "index.html",
  contentFormat: "static",
  bundle: { address: rawContentAddress(bundleBytes), size: bundleBytes.length },
  requestedCapabilities: ["accounts", "camera"],
};

test("OriginAppManifestV1 is canonical, bounded, and round-trips", () => {
  const first = encodeOriginAppManifest(manifest);
  const reordered = encodeOriginAppManifest({ ...manifest, requestedCapabilities: ["camera", "accounts"] });
  assert.deepEqual(first, reordered);
  assert.deepEqual(decodeOriginAppManifest(first), manifest);
  assert.throws(() => encodeOriginAppManifest({ ...manifest, entrypoint: "../index.html" }), /safe relative/);
  assert.throws(
    () => encodeOriginAppManifest({
      ...manifest,
      bundle: { address: rawContentAddress(bundleBytes, "sha2-256"), size: bundleBytes.length },
    }),
    /bundle address is invalid/,
  );
});

test("resolver pins all native authority reads to one finalized block", async () => {
  const bytes = encodeOriginAppManifest(manifest);
  const address = rawContentAddress(bytes);
  const commitment = contentCommitment(`0x${Buffer.from(
    parseContentCid(address.cid).digest,
  ).toString("hex")}`);
  const store = {
    async put() { return { commitment, address }; },
    async get(requested) { assert.equal(requested, commitment); return bytes.slice(); },
  } satisfies OriginAppContentStore;
  const seen: string[] = [];
  const runtime = {
    async nameStatus(hash) { seen.push(`${hash}:status`); return { version: 1, exists: true, active: true, expires_at: "10" as never }; },
    async nameById(hash) { seen.push(`${hash}:name`); return { version: 1, value: { name: appName, parent: null, label: "festival" as never, owner, expires_at: "10" as never, depth: 0 } }; },
    async resolveContentPublication(hash) { seen.push(`${hash}:content`); return { version: 1, value: { content: commitment, revision: 1n } }; },
    async resolveAttestation(hash) { seen.push(`${hash}:attestation`); return { version: 1, value: null }; },
    async publishContent() { return tx; },
  } as NamesRuntimeAdapter;
  const chain = createCommonsChainClient({
    async finalizedBlock() { return { hash: at, number: 7n }; },
    async runtimeIdentity() { return binding; },
    async disconnect() {},
  });
  const resolved = await createOriginAppsClient(chain, runtime, store).resolveApp(appName);
  assert.equal(resolved.success, true);
  assert.deepEqual(seen, [`${at}:status`, `${at}:name`, `${at}:content`, `${at}:attestation`]);
  assert.equal(resolved.success && resolved.value.launch.productId, "festival.app");
  assert.equal(resolved.success && resolved.value.finalized.number, 7n);
});

test("resolver rejects inactive names and manifest/name mismatches", async () => {
  const chain = createCommonsChainClient({
    async finalizedBlock() { return { hash: at, number: 7n }; },
    async runtimeIdentity() { return binding; },
    async disconnect() {},
  });
  const runtime = { async nameStatus() { return { version: 1, exists: true, active: false, expires_at: "6" }; } } as NamesRuntimeAdapter;
  const store = {} as OriginAppContentStore;
  const resolved = await createOriginAppsClient(chain, runtime, store).resolveApp(appName);
  assert.equal(!resolved.success && resolved.error.code, "name_inactive");
});
