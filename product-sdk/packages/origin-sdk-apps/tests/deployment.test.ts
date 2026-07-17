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
import type { CloudStorageClient } from "@cord-network/origin-sdk-cloud-storage";
import { nameId, operationId } from "@cord-network/origin-sdk-names";
import { ok } from "@cord-network/origin-sdk-result";
import type { PreparedTransaction } from "@cord-network/origin-sdk-tx";
import {
  createOriginAppDeployer,
  createOriginStaticPackager,
  type OriginAppBlockStore,
  type OriginAppsClient,
  type StoredOriginAppManifest,
} from "../src/index.ts";

const tx: PreparedTransaction = { async *signSubmitAndWatch() {} };

test("static packaging is deterministic and deployment reuses unchanged blocks", async () => {
  const packager = createOriginStaticPackager();
  const files = [
    { path: "index.html", bytes: new TextEncoder().encode("<h1>Festival</h1>"), mediaType: "text/html" },
    { path: "app.js", bytes: new TextEncoder().encode("export{}"), mediaType: "text/javascript" },
  ];
  const first = await packager.package(files);
  const reordered = await packager.package([...files].reverse());
  assert.equal(first.root.cid, reordered.root.cid);

  const retained = new Set<string>([first.blocks[0]!.commitment]);
  const blockStore = {
    async has(commitment) { return retained.has(commitment); },
    async put(block) { retained.add(block.commitment); },
  } satisfies OriginAppBlockStore;
  const storedManifest = { commitment: first.blocks[0]!.commitment } as StoredOriginAppManifest;
  const apps = {
    async storeManifest(manifest) { return ok({ ...storedManifest, manifest, bytes: new Uint8Array([1]), address: first.root }); },
    async prepareManifestBinding() { return ok(tx); },
  } as OriginAppsClient;
  let writes = 0;
  const storage = {
    async prepare() { writes += 1; return ok(tx); },
  } as CloudStorageClient;
  const deployer = createOriginAppDeployer(apps, storage, blockStore, packager);
  const prepared = await deployer.prepare({
    product: { id: "festival.app", name: "Festival" },
    nameId: nameId(`0x${"22".repeat(32)}`),
    version: "1.0.0",
    channel: "stable",
    entrypoint: "index.html",
    contentFormat: "static",
    requestedCapabilities: ["accounts"],
    publication: { expectedRevision: "0", operationId: operationId(`0x${"33".repeat(16)}`) },
    files,
  });
  assert.equal(prepared.success, true);
  assert.equal(prepared.success && prepared.value.reusedCommitments.length, 1);
  assert.equal(prepared.success && prepared.value.uploadedCommitments.length, 3);
  assert.equal(writes, 3);
});
