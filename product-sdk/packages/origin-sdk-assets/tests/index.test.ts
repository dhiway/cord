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
import { COMMONS_NETWORK_BINDING } from "@cord-network/origin-sdk-descriptors";
import { accountId } from "@cord-network/origin-sdk-identity";
import type { PreparedTransaction } from "@cord-network/origin-sdk-tx";
import {
  ASSETS_ADMIN_EXCLUSIONS,
  ASSETS_DUPLICATE_EXCLUSIONS,
  ASSETS_NATIVE_BINDINGS,
  ASSETS_RUNTIME_GAPS,
  assetId,
  assetReads,
  assetWrites,
  balance,
  collectionId,
  commonsAsset,
  createAssetsClient,
  foundationAsset,
  itemId,
  metadataBytes,
  paymentOptions,
  type AssetsRuntimeAdapter,
} from "../src/index.ts";

const finalizedHash = `0x${"88".repeat(32)}` as const;
const runtimeIdentity = {
  genesis_hash: COMMONS_NETWORK_BINDING.genesis_hash,
  spec_version: COMMONS_NETWORK_BINDING.spec_version,
  transaction_version: COMMONS_NETWORK_BINDING.transaction_version,
  metadata_hash: COMMONS_NETWORK_BINDING.metadata_hash,
  descriptor_contract_sha256: COMMONS_NETWORK_BINDING.descriptor_contract_sha256,
  chain_spec_source_sha256: COMMONS_NETWORK_BINDING.chain_spec_source_sha256,
};
const transaction: PreparedTransaction = { async *signSubmitAndWatch() {} };

test("asset reads and payment-aware writes pin one verified finalized block", async () => {
  const seen: Array<{ at: string; target: string; feeAsset?: unknown }> = [];
  const chain = createCommonsChainClient({
    finalizedBlock: async () => ({ hash: finalizedHash, number: 17n }),
    runtimeIdentity: async () => runtimeIdentity,
    disconnect: async () => {},
  });
  const runtime = {
    async read(at, target) {
      seen.push({ at, target });
      return target;
    },
    async prepare(at, target, _payload, payment) {
      seen.push({ at, target, feeAsset: payment.feeAsset });
      return transaction;
    },
  } satisfies AssetsRuntimeAdapter;
  const client = createAssetsClient(chain, runtime);
  const owner = accountId("5Owner");
  const together = await client.readTogether([
    assetReads.nativeAccount(owner),
    assetReads.details(assetId(7)),
  ]);
  const feeAsset = commonsAsset(assetId(7));
  const prepared = await client.prepare(
    assetWrites.transferNative(owner, balance(10)),
    paymentOptions(feeAsset, 2),
  );

  assert.equal(together.success, true);
  assert.equal(together.success && together.value.finalized_number, 17n);
  assert.equal(prepared.success, true);
  assert.deepEqual(seen.map(({ at, target }) => `${at}:${target}`), [
    `${finalizedHash}:System.Account`,
    `${finalizedHash}:Assets.Asset`,
    `${finalizedHash}:Balances.transfer_keep_alive`,
  ]);
  assert.deepEqual(seen[2]?.feeAsset, feeAsset);
});

test("asset constructors enforce Commons bounds before transport", () => {
  const owner = accountId("5Owner");
  const local = commonsAsset(assetId(1));
  const foundation = foundationAsset();

  assert.deepEqual(foundation, { parents: 1, junctions: [] });
  assert.equal(assetWrites.burn(assetId(1), owner, balance(2)).payload.who, owner);
  assert.equal(
    assetWrites.mintNft(collectionId(1), itemId(2), owner).target,
    "Nfts.mint",
  );
  assert.throws(() => metadataBytes(new Uint8Array(51)), /at most 50 bytes/);
  assert.throws(
    () => assetWrites.setMetadata(assetId(1), new Uint8Array(), new Uint8Array(), 256),
    /u8/,
  );
  assert.throws(
    () => assetWrites.swapExactInput(
      [foundation, local, foundation, local, foundation], balance(1), balance(1), owner,
    ),
    /2-4 locations/,
  );
});

test("native bindings make exclusions and the quote gap explicit", () => {
  assert.equal(ASSETS_NATIVE_BINDINGS.maxConversionPathLength, 4);
  assert.equal(ASSETS_NATIVE_BINDINGS.assetMetadataStringLimit, 50);
  assert.equal(ASSETS_NATIVE_BINDINGS.canonicalNft, "Nfts");
  assert.equal(ASSETS_NATIVE_BINDINGS.writes.length, 27);
  assert.equal(ASSETS_ADMIN_EXCLUSIONS.length, 9);
  assert.equal(ASSETS_DUPLICATE_EXCLUSIONS[0].target, "Uniques.*");
  assert.equal(ASSETS_RUNTIME_GAPS[0].target, "AssetConversionApi.*");
});
