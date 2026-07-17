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
import { NATIVE_HOST_METHODS } from "../packages/descriptors/src/native-methods.ts";

const sdkRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const repoRoot = resolve(sdkRoot, "..");
const sourceHeader = readFileSync(resolve(repoRoot, "HEADER-GPL3"), "utf8");
const rel = {
  compatibility: "docs/sdk/compatibility-manifest.json",
  versionMatrix: "docs/sdk/native-version-matrix.json",
  extensions: "docs/sdk/signed-extension-manifest.json",
  metadata: "origin/orbis/runtime/vectors/transaction-policy-v8/metadata-hash.json",
  metadataScale: "product-sdk/packages/descriptors/.papi/metadata/commons.scale",
  papiDescriptor: "product-sdk/packages/descriptors/generated/commons-papi-manifest.json",
  vectors: "docs/sdk/vectors/native-sdk-v1.json",
  genesisIdentity: "docs/genesis/orbis-candidate-genesis-identity.json",
  chainSpec: "origin/orbis/node/src/chain_spec.rs",
  nativeRouteContract: "docs/sdk/native-route-contract.json",
  ratification: "docs/evidence/verification/p5/sdk-freeze-ratification-envelope.json"
};
const readJson = (path: string) => JSON.parse(readFileSync(resolve(repoRoot, path), "utf8"));
const sha256 = (path: string) => createHash("sha256").update(readFileSync(resolve(repoRoot, path))).digest("hex");
const canonical = (value: any): string => value === null || typeof value !== "object" ? JSON.stringify(value) : Array.isArray(value) ? `[${value.map(canonical).join(",")}]` : `{${Object.keys(value).sort().map((key) => `${JSON.stringify(key)}:${canonical(value[key])}`).join(",")}}`;
const sourceDigest = (name: string, path: string) => {
  if (["compatibility", "extensions"].includes(name)) { const value=readJson(path); delete value.ratification; return createHash("sha256").update(canonical(value)).digest("hex"); }
  return sha256(path);
};
const hostSchemaPath = resolve(repoRoot, "docs/sdk/host/host-request.schema.json");
const projectNativeHostRoutes = (hostSchema: any) => {
  hostSchema.properties.method.enum = [...new Set(NATIVE_HOST_METHODS.map(({ method }) => method))].sort();
  hostSchema.oneOf = NATIVE_HOST_METHODS.map(({ capability, method, finality, payloadFields }) => ({
    properties: {
      capability: { const: capability },
      method: { const: method },
      finality: { const: finality },
      payload: {
        type: "object",
        additionalProperties: false,
        required: payloadFields,
        properties: Object.fromEntries(payloadFields.map((field) => [field, {}]))
      }
    },
    required: ["capability", "method", "finality", "payload"]
  }));
  hostSchema.$comment = "Payload field types and bounds are enforced by product-sdk/packages/core/src/contract.ts; this schema freezes exact closed shapes, semantic routes and finality.";
  return hostSchema;
};
if (process.argv.includes("--host-schema-only")) {
  const current = readFileSync(hostSchemaPath, "utf8");
  const serialized = `${JSON.stringify(projectNativeHostRoutes(JSON.parse(current)), null, 2)}\n`;
  if (process.argv.includes("--check")) {
    if (current !== serialized)
      throw new Error("host request route schema drift; run generate-descriptor.ts --host-schema-only");
    process.stdout.write(`PASS host request route schema: ${NATIVE_HOST_METHODS.length} methods\n`);
  } else {
    writeFileSync(hostSchemaPath, serialized);
    process.stdout.write(`${hostSchemaPath}\n`);
  }
  process.exit(0);
}
const compatibility = readJson(rel.compatibility);
const extensions = readJson(rel.extensions);
const versionMatrix = readJson(rel.versionMatrix);
const metadata = readJson(rel.metadata);
const vectors = readJson(rel.vectors);
const metadataIdentity = readJson("docs/sdk/metadata/commons-v29.json");
const papiAvailable = metadataIdentity.spec_version === metadata.spec_version;
const ratification = readJson(rel.ratification);
const productionActivationReady = ratification.derived_status.production_activation_ready === true;
const finalGenesisStatus = ratification.payload.production_activation.final_genesis_status;
if ((!productionActivationReady && finalGenesisStatus !== "PENDING")
  || (productionActivationReady && finalGenesisStatus === "PENDING"))
  throw new Error("ratification activation state is internally inconsistent");
const activationState = productionActivationReady ? "production-approved" : "candidate-pending";

const values = [
  compatibility.network.orbis_spec_version,
  extensions.runtime.spec_version,
  metadata.spec_version,
  vectors.runtime.spec_version
];
const transactions = [
  compatibility.network.orbis_transaction_version,
  extensions.runtime.transaction_version,
  metadata.transaction_version,
  vectors.runtime.transaction_version
];
if (!values.every((value) => value === versionMatrix.networks.orbis.spec_version) || !transactions.every((value) => value === versionMatrix.networks.orbis.transaction_version)) {
  throw new Error(`Orbis version drift: spec=${values.join(",")}, transaction=${transactions.join(",")}`);
}
if (vectors.runtime.metadata_hash !== metadata.metadata_hash || versionMatrix.networks.orbis.metadata_hash !== metadata.metadata_hash) throw new Error("metadata hash drift between native vector, matrix and metadata manifests");

const descriptor = {
  contractVersion: 1,
  kind: "cord-native-host-contract-manifest",
  release: versionMatrix.release,
  firstSupportedNativeSdk: true,
  runtime: {
    name: "orbis",
    paraId: compatibility.network.para_id,
    specVersion: versionMatrix.networks.orbis.spec_version,
    transactionVersion: versionMatrix.networks.orbis.transaction_version,
    metadataHash: metadata.metadata_hash
  },
  fixtureIdentity: ratification.payload.fixture_identity,
  networkActivation: {
    state: activationState,
    productionActivationReady,
    source: rel.ratification,
  },
  ratificationPayloadSha256: ratification.payload_sha256,
  sources: Object.fromEntries(Object.entries(rel).filter(([name])=>name!=="ratification").map(([name, path]) => [name, { path, sha256: sourceDigest(name,path) }])),
  signedExtensionSurfaces: Object.fromEntries(
    Object.entries(extensions.surfaces).map(([name, surface]: [string, any]) => [name, surface.extensions])
  ),
  nativeHostContract: {
    version: 1,
    methodCount: NATIVE_HOST_METHODS.length,
    pageLimit: 100,
    payloadValidation: "closed-shape-plus-core-native-types-v1",
    methods: NATIVE_HOST_METHODS
  },
  descriptorProvenance: {
  runtimeMetadataBinding: papiAvailable
    ? "checked-in-v14-scale-plus-reproduced-rfc78-wasm-metadata-hash"
    : "unavailable-checked-in-papi-metadata-predates-current-runtime",
    papiDescriptor: "polkadot-api-2.1.6-byte-reproducible-generation",
    methodInventory: "authoritative-typed-native-route-contract",
    driftValidation: "metadata-hash-pallet-call-index-runtime-api-and-rust-typescript-route-harness",
  },
  papiAvailability: {
    runtimeMetadataCurrent: papiAvailable,
    sdkAdmission: papiAvailable,
    reason: papiAvailable ? null : "checked-in PAPI metadata predates the current Commons runtime",
  },
  productionPapiDescriptorGenerated: papiAvailable
};
const serialized = `${JSON.stringify(descriptor, null, 2)}\n`;
const output = resolve(sdkRoot, "packages/descriptors/generated/orbis-descriptor.json");
const descriptorContract={...descriptor};delete (descriptorContract as any).ratificationPayloadSha256;
const descriptorContractSha256 = createHash("sha256").update(canonical(descriptorContract)).digest("hex");
const bindingOutput = resolve(sdkRoot, "packages/descriptors/generated/orbis-network-binding.ts");
const packageBindingOutput = resolve(sdkRoot, "packages/origin-sdk-descriptors/src/network-binding.ts");
const binding = {
  genesis_hash: descriptor.fixtureIdentity.genesis_identity,
  spec_version: descriptor.runtime.specVersion,
  transaction_version: descriptor.runtime.transactionVersion,
  metadata_hash: descriptor.runtime.metadataHash,
  descriptor_contract_sha256: descriptorContractSha256,
  chain_spec_source_sha256: descriptor.fixtureIdentity.chain_spec_source_sha256,
  activation_state: activationState,
  production_activation_ready: productionActivationReady,
};
const bindingSerialized = `${sourceHeader}\n// Generated by tools/generate-descriptor.ts. Do not edit.\nexport const ORBIS_NETWORK_BINDING = ${JSON.stringify(binding, null, 2)} as const;\n\nexport const ORBIS_CANDIDATE_NETWORK_BINDING = {\n  ...ORBIS_NETWORK_BINDING,\n  access_mode: "candidate",\n} as const;\n`;
const packageBindingSerialized = `${sourceHeader}\n// Generated by tools/generate-descriptor.ts. Do not edit.\nexport const COMMONS_NETWORK_BINDING = ${JSON.stringify(binding, null, 2)} as const;\n\nexport const COMMONS_CANDIDATE_NETWORK_BINDING = {\n  ...COMMONS_NETWORK_BINDING,\n  access_mode: "candidate",\n} as const;\n`;
const hostSchema = JSON.parse(readFileSync(hostSchemaPath, "utf8"));
hostSchema.properties.network.properties.genesis_hash = { const: descriptor.fixtureIdentity.genesis_identity };
hostSchema.properties.network.properties.spec_version = { const: descriptor.runtime.specVersion };
hostSchema.properties.network.properties.transaction_version = { const: descriptor.runtime.transactionVersion };
hostSchema.properties.network.properties.metadata_hash = { const: descriptor.runtime.metadataHash };
hostSchema.properties.network.properties.chain_spec_source_sha256 = { const: descriptor.fixtureIdentity.chain_spec_source_sha256 };
hostSchema.properties.network.properties.descriptor_contract_sha256 = { const: descriptorContractSha256 };
hostSchema.properties.network.properties.activation_state = { const: activationState };
hostSchema.properties.network.properties.production_activation_ready = { const: productionActivationReady };
hostSchema.properties.network.properties.access_mode = { enum: ["candidate", "production"] };
hostSchema.properties.network.required = [...new Set([
  ...hostSchema.properties.network.required,
  "activation_state",
  "production_activation_ready",
  "access_mode",
])];
projectNativeHostRoutes(hostSchema);
const hostSerialized = `${JSON.stringify(hostSchema, null, 2)}\n`;
if (process.argv.includes("--check")) {
  if (readFileSync(output, "utf8") !== serialized) throw new Error("descriptor drift; run npm run generate:descriptors");
  if (readFileSync(bindingOutput, "utf8") !== bindingSerialized) throw new Error("network binding drift; run npm run generate:descriptors");
  if (readFileSync(packageBindingOutput, "utf8") !== packageBindingSerialized) throw new Error("package network binding drift; run npm run generate:descriptors");
  if (readFileSync(hostSchemaPath, "utf8") !== hostSerialized) throw new Error("host schema drift; run npm run generate:descriptors");
  process.stdout.write(`PASS descriptor contract is current for Orbis spec ${descriptor.runtime.specVersion} / transaction ${descriptor.runtime.transactionVersion}\n`);
} else {
  mkdirSync(dirname(output), { recursive: true });
  writeFileSync(output, serialized);
  writeFileSync(bindingOutput, bindingSerialized);
  mkdirSync(dirname(packageBindingOutput), { recursive: true });
  writeFileSync(packageBindingOutput, packageBindingSerialized);
  writeFileSync(hostSchemaPath, hostSerialized);
  process.stdout.write(`${output}\n`);
}
