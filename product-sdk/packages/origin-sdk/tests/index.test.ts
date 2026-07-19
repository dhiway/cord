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
import { COMMONS_NETWORK_BINDING } from "@cord-network/origin-sdk-descriptors";
import { createFakeHost } from "@cord-network/origin-sdk-host/testing";
import { utf8Codec } from "@cord-network/origin-sdk-local-storage";
import {
  ORIGIN_APP_CONTRACT,
  createApp,
  type OriginAppRuntime,
} from "../src/index.ts";

const product = { id: "festival.app", name: "Festival" } as const;
const runtime = {
  identity: {}, personhood: {}, resources: {}, attestation: {}, names: {}, storage: {}, assets: {},
} as OriginAppRuntime;
const runtimeIdentity = {
  genesis_hash: COMMONS_NETWORK_BINDING.genesis_hash,
  spec_version: COMMONS_NETWORK_BINDING.spec_version,
  transaction_version: COMMONS_NETWORK_BINDING.transaction_version,
  metadata_hash: COMMONS_NETWORK_BINDING.metadata_hash,
  descriptor_contract_sha256: COMMONS_NETWORK_BINDING.descriptor_contract_sha256,
  chain_spec_source_sha256: COMMONS_NETWORK_BINDING.chain_spec_source_sha256,
};

function readyHost(identity = runtimeIdentity) {
  const fake = createFakeHost({ accounts: [{ address: "5Festival" }], runtimeIdentity: identity });
  for (const capability of ["chain", "accounts", "signing", "local-storage", "statements"] as const) {
    fake.grant(product.id, capability);
  }
  return fake;
}

test("createApp validates Commons and wires the hosted developer surface once", async () => {
  const fake = readyHost();
  const created = await createApp({ product, bridge: fake.bridge, runtime });
  assert.equal(created.success, true);
  if (!created.success) return;
  assert.equal(created.value.signer.account.address, "5Festival");
  assert.deepEqual(ORIGIN_APP_CONTRACT.nativeDomains, [
    "identity", "personhood", "resources", "attestations", "names",
    "cloudStorage", "statements", "assets",
  ]);
  assert.equal(ORIGIN_APP_CONTRACT.contractsIncluded, false);
  assert.equal((await created.value.storage.set("theme", "dark", utf8Codec)).success, true);
  const theme = await created.value.storage.get("theme", utf8Codec);
  assert.equal(theme.success && theme.value, "dark");
  assert.equal((await created.value.close()).success, true);
  assert.equal(created.value.signal.aborted, true);
  assert.equal((await created.value.close()).success, true);
});

test("createApp fails before exposing clients on wrong genesis or metadata drift", async () => {
  const wrongGenesis = readyHost({ ...runtimeIdentity, genesis_hash: `0x${"ff".repeat(32)}` });
  const wrong = await createApp({ product, bridge: wrongGenesis.bridge, runtime });
  assert.equal(!wrong.success && wrong.error.code, "runtime_identity_mismatch");
  assert.equal(!wrong.success && wrong.error.details?.field, "genesis_hash");

  const drifted = readyHost({ ...runtimeIdentity, metadata_hash: `0x${"ee".repeat(32)}` });
  const drift = await createApp({ product, bridge: drifted.bridge, runtime });
  assert.equal(!drift.success && drift.error.code, "runtime_identity_mismatch");
  assert.equal(!drift.success && drift.error.details?.field, "metadata_hash");
});

test("createApp requires host-owned chain and account permissions", async () => {
  const fake = createFakeHost({ accounts: [{ address: "5Festival" }], runtimeIdentity });
  const denied = await createApp({ product, bridge: fake.bridge, runtime });
  assert.equal(!denied.success && denied.error.code, "permission_denied");
});
