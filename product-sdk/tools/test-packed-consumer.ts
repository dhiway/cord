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

import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { resolve } from "node:path";
import { spawnSync } from "node:child_process";

const sdkRoot = resolve(import.meta.dirname, "..");
const consumer = mkdtempSync(resolve(tmpdir(), "cord-origin-sdk-consumer-"));
const packages = ["result", "errors", "descriptors", "host", "chain-client", "signer", "tx", "identity", "personhood", "resources", "attestation", "crypto", "names"];
const run = (command: string, args: string[], cwd = consumer): string => {
  const result = spawnSync(command, args, { cwd, encoding: "utf8" });
  if (result.status !== 0) {
    process.stderr.write(result.stdout ?? "");
    process.stderr.write(result.stderr ?? "");
    throw new Error(`${command} ${args.join(" ")} failed`);
  }
  return result.stdout;
};

try {
  const dependencies: Record<string, string> = { "polkadot-api": "2.1.6" };
  for (const suffix of packages) {
    const packageRoot = resolve(sdkRoot, `packages/origin-sdk-${suffix}`);
    const manifest = JSON.parse(readFileSync(resolve(packageRoot, "package.json"), "utf8"));
    const packed = JSON.parse(run("npm", ["pack", packageRoot, "--pack-destination", consumer, "--json"]));
    dependencies[manifest.name] = `file:${resolve(consumer, packed[0].filename)}`;
  }
  writeFileSync(resolve(consumer, "package.json"), `${JSON.stringify({
    name: "cord-origin-sdk-packed-consumer",
    private: true,
    type: "module",
    dependencies,
  }, null, 2)}\n`);
  writeFileSync(resolve(consumer, "index.mjs"), `
import { COMMONS_NETWORK_BINDING } from "@cord-network/origin-sdk-descriptors";
import { createCommonsChainClient } from "@cord-network/origin-sdk-chain-client";
import { createHostClient } from "@cord-network/origin-sdk-host";
import { createFakeHost } from "@cord-network/origin-sdk-host/testing";
import { createHostSigner } from "@cord-network/origin-sdk-signer";
import { submitAndFinalize } from "@cord-network/origin-sdk-tx";
import { accountId, createIdentityClient } from "@cord-network/origin-sdk-identity";
import { blake2b256 } from "@cord-network/origin-sdk-crypto";
import { normalizedLabel } from "@cord-network/origin-sdk-names";
const hash = "0x" + "11".repeat(32), txHash = "0x" + "22".repeat(32);
if (blake2b256(new Uint8Array()).length !== 32 || normalizedLabel("packed-app") !== "packed-app") throw new Error("packed crypto/names surface failed");
const identity = {
  genesis_hash: COMMONS_NETWORK_BINDING.genesis_hash,
  spec_version: COMMONS_NETWORK_BINDING.spec_version,
  transaction_version: COMMONS_NETWORK_BINDING.transaction_version,
  metadata_hash: COMMONS_NETWORK_BINDING.metadata_hash,
  descriptor_contract_sha256: COMMONS_NETWORK_BINDING.descriptor_contract_sha256,
  chain_spec_source_sha256: COMMONS_NETWORK_BINDING.chain_spec_source_sha256,
};
const chain = createCommonsChainClient({ finalizedBlock: async () => ({ hash, number: 1n }), runtimeIdentity: async () => identity, disconnect: async () => {} });
const read = await chain.readFinalized(async (at) => at);
if (!read.success || read.value !== hash) throw new Error("packed finalized read failed");
const identityClient = createIdentityClient(chain, {
  identityStatus: async (at, account) => ({ version: 1, value: { registered: at === hash && account === "5Packed", judgement_count: 0, requested: 0, reasonable: 0, known_good: 0, out_of_date: 0, low_quality: 0, erroneous: 0 } }),
  setIdentity: async () => { throw new Error("not used"); }, clearIdentity: async () => { throw new Error("not used"); },
  requestJudgement: async () => { throw new Error("not used"); }, cancelJudgementRequest: async () => { throw new Error("not used"); }, provideJudgement: async () => { throw new Error("not used"); },
});
const identityStatus = await identityClient.status(accountId("5Packed"));
if (!identityStatus.success || !identityStatus.value.value?.registered) throw new Error("packed identity read failed");
const fake = createFakeHost({ accounts: [{ address: "5Packed" }] });
fake.grant("packed.app", "signing");
const signer = createHostSigner(createHostClient(fake.bridge, { id: "packed.app", name: "Packed" }));
const transaction = { async *signSubmitAndWatch(activeSigner) { const signed = await activeSigner.sign({ account: "5Packed", payload: new Uint8Array([1]), purpose: "packed-test" }); if (!signed.success) throw signed.error; yield { type: "finalized", blockHash: hash, transactionHash: txHash }; } };
const receipt = await submitAndFinalize(transaction, signer);
if (!receipt.success || receipt.value.transactionHash !== txHash) throw new Error("packed signed transaction failed");
`);
  run("npm", ["install", "--ignore-scripts", "--no-audit", "--no-fund"]);
  run("node", ["index.mjs"]);
  process.stdout.write(`PASS packed consumer: packages=${packages.length}\n`);
} finally {
  rmSync(consumer, { recursive: true, force: true });
}
