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
const packages = ["result", "errors", "descriptors", "host", "chain-client", "signer", "tx", "identity", "attestation", "crypto", "names", "cloud-storage", "apps", "assets", "local-storage", ""];
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
    const packageRoot = resolve(sdkRoot, suffix ? `packages/origin-sdk-${suffix}` : "packages/origin-sdk");
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
import { digestContent, storageRequests } from "@cord-network/origin-sdk-cloud-storage";
import { ORIGIN_APP_MANIFEST_VERSION } from "@cord-network/origin-sdk-apps";
import { assetId, assetWrites, balance, commonsAsset, paymentOptions } from "@cord-network/origin-sdk-assets";
import { createLocalStorage, utf8Codec } from "@cord-network/origin-sdk-local-storage";
import { ORIGIN_APP_CONTRACT } from "@cord-network/origin-sdk";
const hash = "0x" + "11".repeat(32), txHash = "0x" + "22".repeat(32);
const packedAsset = assetWrites.transfer(assetId(7), accountId("5Packed"), balance(9));
const packedPayment = paymentOptions(commonsAsset(assetId(7)));
if (ORIGIN_APP_CONTRACT.contractsIncluded !== false || ORIGIN_APP_MANIFEST_VERSION !== 1 || blake2b256(new Uint8Array()).length !== 32 || normalizedLabel("packed-app") !== "packed-app" || digestContent("blake2b-256", new Uint8Array()).length !== 32 || storageRequests.store("YQ==").method !== "store" || packedAsset.target !== "Assets.transfer" || packedPayment.feeAsset?.parents !== 0) throw new Error("packed apps/crypto/names/storage/assets surface failed");
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
const packedHost = createHostClient(fake.bridge, { id: "packed.app", name: "Packed" });
fake.grant("packed.app", "local-storage");
const packedStorage = createLocalStorage(packedHost, "quickstart");
if (!(await packedStorage.set("ready", "yes", utf8Codec)).success || (await packedStorage.get("ready", utf8Codec)).value !== "yes") throw new Error("packed local storage failed");
const signer = createHostSigner(packedHost);
const transaction = { async *signSubmitAndWatch(activeSigner) { const signed = await activeSigner.sign({ account: "5Packed", payload: new Uint8Array([1]), purpose: "packed-test" }); if (!signed.success) throw signed.error; yield { type: "finalized", blockHash: hash, transactionHash: txHash }; } };
const receipt = await submitAndFinalize(transaction, signer);
if (!receipt.success || receipt.value.transactionHash !== txHash) throw new Error("packed signed transaction failed");
`);
  writeFileSync(resolve(consumer, "private-policy.mjs"), `
const entrypoints = [
  "@cord-network/origin-sdk-host",
  "@cord-network/origin-sdk-cloud-storage",
  "@cord-network/origin-sdk-identity",
  "@cord-network/origin-sdk-apps",
  "@cord-network/origin-sdk-descriptors",
  "@cord-network/origin-sdk",
];
const forbidden = [
  "@cord-network/origin-sdk-host/v2",
  "@cord-network/origin-sdk-host/internal/v2",
  "@cord-network/origin-sdk-host/internal/v2/browser",
  "@cord-network/origin-sdk-cloud-storage/v2",
  "@cord-network/origin-sdk-cloud-storage/internal/storage-v2-codec",
  "@cord-network/origin-sdk-cloud-storage/internal/storage-v2-intents",
  "@cord-network/origin-sdk-identity/v2",
  "@cord-network/origin-sdk-identity/internal/v2",
  "@cord-network/origin-sdk-apps/v2",
  "@cord-network/origin-sdk-apps/internal/v2",
  "@cord-network/origin-sdk-descriptors/host-v2",
  "@cord-network/origin-sdk-descriptors/internal/host-v2-descriptor",
  "@cord-network/origin-sdk/v2",
  "@cord-network/origin-sdk/internal/v2",
];
for (const specifier of forbidden) {
  try {
    await import(specifier);
    throw new Error(\`private subpath became importable: \${specifier}\`);
  } catch (error) {
    if (error instanceof Error && error.message.startsWith("private subpath became importable")) throw error;
    if (error?.code !== "ERR_PACKAGE_PATH_NOT_EXPORTED") {
      throw new Error(\`private subpath did not fail through the package export boundary: \${specifier}: \${error?.code ?? error}\`);
    }
  }
}
for (const specifier of entrypoints) {
  const publicSurface = await import(specifier);
  const leaked = Object.keys(publicSurface).filter((name) => /v2/i.test(name));
  if (leaked.length > 0) throw new Error(\`private v2 runtime symbols leaked from \${specifier}: \${leaked.join(", ")}\`);
}
`);
  run("npm", ["install", "--ignore-scripts", "--no-audit", "--no-fund"]);
  run("node", ["index.mjs"]);
  run("node", ["private-policy.mjs"]);

  const allowedExports: Readonly<Record<string, readonly string[]>> = {
    "@cord-network/origin-sdk-host": [".", "./testing"],
    "@cord-network/origin-sdk-cloud-storage": ["."],
    "@cord-network/origin-sdk-identity": ["."],
    "@cord-network/origin-sdk-apps": ["."],
    "@cord-network/origin-sdk-descriptors": [".", "./commons"],
    "@cord-network/origin-sdk": [".", "./testing"],
  };
  for (const [name, expected] of Object.entries(allowedExports)) {
    const packageRoot = resolve(consumer, "node_modules", ...name.split("/"));
    const manifest = JSON.parse(readFileSync(resolve(packageRoot, "package.json"), "utf8"));
    const actual = Object.keys(manifest.exports ?? {}).sort();
    if (JSON.stringify(actual) !== JSON.stringify([...expected].sort())) {
      throw new Error(`${name} exports changed: expected ${expected.join(", ")}; got ${actual.join(", ")}`);
    }
    const declarations = readFileSync(resolve(packageRoot, "dist/index.d.ts"), "utf8");
    if (/\b[A-Za-z][A-Za-z0-9_]*V2[A-Za-z0-9_]*\b|\/internal\/|(?:^|["'])\.\/v2(?:["']|$)/m.test(declarations)) {
      throw new Error(`${name} root declarations leak a private v2 symbol or path`);
    }
  }
  const umbrella = JSON.parse(readFileSync(resolve(consumer, "node_modules/@cord-network/origin-sdk/package.json"), "utf8"));
  if (umbrella.dependencies?.["@cord-network/origin-sdk-apps"] !== "0.1.0") {
    throw new Error("packed umbrella does not retain its exact origin-sdk-apps dependency");
  }
  for (const retired of ["contracts", "personhood", "resources", "statement-store"]) {
    if (`@cord-network/origin-sdk-${retired}` in (umbrella.dependencies ?? {})) {
      throw new Error(`packed umbrella retained retired ${retired} dependency`);
    }
  }
  process.stdout.write(`PASS packed consumer and private-v2 export policy: packages=${packages.length} forbidden=${14}\n`);
} finally {
  rmSync(consumer, { recursive: true, force: true });
}
