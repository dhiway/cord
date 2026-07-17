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

/** Private descriptor projection for replacement-first host v2 integration. */

export const HOST_V2_PROTOCOL = "cord.origin.host/2" as const;
export const HOST_V2_MAJOR = 2 as const;
export const HOST_V2_MINOR = 0 as const;

export const HOST_V2_SOURCE_HASHES = {
  cddl: "71d71f02b7c1b4e55892c88bb6cdeba53852f981625f7bf831b79c97520264e3",
  operations: "48934f781f5bd7f377684a5f17dd5eb3d9efa2b178b408cb0db80eb5b3d738b1",
  errors: "5bce2efd0c2c84d9bd09f568100467ffb2681a42e4f405746937c9651c6dc0eb",
  schema: "ab087de1993230668ca5d37f5fecb15fdc3c71bd4d8fe10819b0f920a12b6a47",
} as const;

export const HOST_V2_RUNTIME_SOURCE_HASHES = {
  metadataIdentity: "8c9dc98ce3f2578031be5bf1be7291d3118fc0b6e79c81f5fee97d73ca53ba43",
  metadataScale: "2785e1e501df231ea313a7613c7cadcdad550c79ecf87bb3797b61c64be462fe",
  papiManifest: "673d04218c99e6c17630a9fedf7689fa8b06761219c92a5e6f9b92ea25a8c595",
} as const;

export const LEGACY_PUBLIC_DESCRIPTOR_HASHES = {
  packageIndex: "5a416585aafda8454366fb48126617e59a0063cc0617c971a14cebf347304cd1",
  packageManifest: "fb9a653b3b1894d096e457724f2182d3443be8965c9249f9566e7adf7b38e1e2",
  runtimeDescriptor: "6892d39913d2a755aec8fb129c3d8529b500884f934520102ab129896f174584",
  networkBinding: "b3295d484e146e147b2e00edba5023db89f90de12507470845ebd073ad804bf1",
} as const;

export type HostV2OperationSignature = readonly [
  code: number,
  name: string,
  feature: string,
  grantScope: string,
];

export const HOST_V2_OPERATION_SIGNATURES = [
  [1000, "storage.bucket.create", "storage.control", "storage.bucket.admin"],
  [1001, "storage.bucket.get", "storage.control", "storage.bucket.read"],
  [1002, "storage.bucket.grant", "storage.control", "storage.bucket.admin"],
  [1003, "storage.bucket.revoke", "storage.control", "storage.bucket.admin"],
  [1010, "storage.object.put", "storage.content", "storage.bucket.writer"],
  [1011, "storage.object.get", "storage.content", "storage.bucket.reader"],
  [1012, "storage.object.range", "storage.content", "storage.bucket.reader"],
  [1013, "storage.object.delete", "storage.deletion", "storage.bucket.writer"],
  [1014, "storage.object.status", "storage.content", "storage.bucket.reader"],
  [1020, "storage.checkpoint.status", "storage.proof", "storage.bucket.reader"],
  [1021, "storage.checkpoint.subscribe", "storage.proof", "storage.bucket.reader"],
  [1022, "storage.replica.status", "storage.replica", "storage.bucket.reader"],
  [1023, "storage.replica.subscribe", "storage.replica", "storage.bucket.reader"],
  [1024, "storage.deletion.status", "storage.deletion", "storage.bucket.writer"],
  [1025, "storage.deletion.subscribe", "storage.deletion", "storage.bucket.writer"],
  [1030, "storage.drive.read", "storage.drive", "storage.bucket.reader"],
  [1031, "storage.drive.commit", "storage.drive", "storage.bucket.writer"],
  [1032, "storage.drive.share", "storage.drive", "storage.bucket.admin"],
  [1040, "storage.s3.put", "storage.s3", "storage.bucket.writer"],
  [1041, "storage.s3.get", "storage.s3", "storage.bucket.reader"],
  [1042, "storage.s3.list", "storage.s3", "storage.bucket.reader"],
  [1043, "storage.s3.delete", "storage.s3", "storage.bucket.writer"],
  [1050, "storage.publish", "storage.publish", "storage.publish"],
  [1051, "storage.resolve", "storage.publish", "public"],
  [1060, "storage.keys.export", "storage.encryption", "storage.keys.export"],
  [1061, "storage.keys.import", "storage.encryption", "storage.keys.import"],
  [1100, "identity.account", "identity.account", "identity.account"],
  [1101, "identity.profile.read", "identity.profile", "identity.profile.read"],
  [1102, "identity.profile.disclose", "identity.profile", "identity.profile.disclose"],
  [1103, "identity.humanity.status", "identity.humanity", "identity.humanity.status"],
  [1104, "identity.humanity.prove", "identity.humanity", "identity.humanity.prove"],
  [1105, "identity.subject.derive", "identity.subject", "identity.subject.derive"],
  [1106, "identity.entitlements.read", "identity.entitlements", "identity.entitlements.read"],
  [1200, "transaction.sign", "transaction.sign", "transaction.sign"],
] as const satisfies readonly HostV2OperationSignature[];

export const HOST_V2_REQUIRED_NEGOTIATION_FEATURES = [
  "identity.account",
  "identity.entitlements",
  "identity.humanity",
  "identity.profile",
  "identity.subject",
  "storage.content",
  "storage.control",
  "storage.deletion",
  "storage.drive",
  "storage.encryption",
  "storage.proof",
  "storage.publish",
  "storage.replica",
  "storage.s3",
  "transaction.sign",
] as const;

export interface FrozenHostV2Operation {
  readonly code: number;
  readonly name: string;
  readonly feature_id: string;
  readonly grant_scope: string;
  readonly consent_mode: string;
  readonly state_changing: boolean;
  readonly operation_id_required: boolean;
  readonly resume: string;
  readonly cancellation: string;
}

export interface FrozenHostV2Operations {
  readonly protocol: string;
  readonly major: number;
  readonly minor: number;
  readonly operations: readonly FrozenHostV2Operation[];
}

export interface FrozenHostV2Error {
  readonly family: string;
  readonly code: number;
  readonly name: string;
  readonly retryable: boolean;
}

export interface FrozenHostV2Errors {
  readonly encoding: string;
  readonly unknown: unknown;
  readonly errors: readonly FrozenHostV2Error[];
}

export interface FrozenCommonsMetadata {
  readonly schema: "cord.commons-runtime-metadata.v1";
  readonly runtime: "commons";
  readonly spec_name: "commons";
  readonly spec_version: number;
  readonly transaction_version: number;
  readonly runtime_metadata_version: number;
  readonly scale_path: string;
  readonly scale_bytes: number;
  readonly scale_sha256: string;
  readonly runtime_rfc78_hash: string;
  readonly genesis_identity: string;
  readonly generator: string;
}

export interface FrozenPapiManifest {
  readonly schema: "cord.commons-papi-descriptor.v1";
  readonly generator: { readonly package: string; readonly version: string };
  readonly metadata: {
    readonly path: string;
    readonly scale_bytes: number;
    readonly scale_sha256: string;
    readonly runtime_metadata_version: number;
    readonly runtime_rfc78_hash: string;
  };
  readonly entry: string;
}

export interface FrozenCommonsNetworkBinding {
  readonly genesis_hash: string;
  readonly spec_version: number;
  readonly transaction_version: number;
  readonly metadata_hash: string;
  readonly descriptor_contract_sha256: string;
  readonly chain_spec_source_sha256: string;
  readonly activation_state: string;
  readonly production_activation_ready: boolean;
}

export interface ProjectedHostV2Operation {
  readonly code: number;
  readonly name: string;
  readonly feature: string;
  readonly grantScope: string;
  readonly consentMode: string;
  readonly stateChanging: boolean;
  readonly operationIdRequired: boolean;
  readonly resume: string;
  readonly cancellation: string;
}

function projectOperation(operation: FrozenHostV2Operation): ProjectedHostV2Operation {
  return {
    code: operation.code,
    name: operation.name,
    feature: operation.feature_id,
    grantScope: operation.grant_scope,
    consentMode: operation.consent_mode,
    stateChanging: operation.state_changing,
    operationIdRequired: operation.operation_id_required,
    resume: operation.resume,
    cancellation: operation.cancellation,
  };
}

function assertFrozenOperations(document: FrozenHostV2Operations): void {
  if (document.protocol !== HOST_V2_PROTOCOL || document.major !== HOST_V2_MAJOR || document.minor !== HOST_V2_MINOR) {
    throw new TypeError("host v2 protocol version drift");
  }
  if (document.operations.length !== HOST_V2_OPERATION_SIGNATURES.length) {
    throw new TypeError(`host v2 operation count drift: ${document.operations.length}`);
  }
  const codes = new Set<number>();
  const names = new Set<string>();
  for (let index = 0; index < HOST_V2_OPERATION_SIGNATURES.length; index += 1) {
    const actual = document.operations[index];
    const expected = HOST_V2_OPERATION_SIGNATURES[index];
    if (actual === undefined || expected === undefined
      || actual.code !== expected[0] || actual.name !== expected[1]
      || actual.feature_id !== expected[2] || actual.grant_scope !== expected[3]) {
      throw new TypeError(`host v2 operation signature drift at index ${index}`);
    }
    if (codes.has(actual.code) || names.has(actual.name)) throw new TypeError("duplicate host v2 operation");
    codes.add(actual.code);
    names.add(actual.name);
  }
}

function projectRuntimeBinding(
  metadata: FrozenCommonsMetadata,
  papi: FrozenPapiManifest,
  network: FrozenCommonsNetworkBinding,
) {
  if (metadata.schema !== "cord.commons-runtime-metadata.v1"
    || metadata.runtime !== "commons" || metadata.spec_name !== "commons") {
    throw new TypeError("Commons runtime metadata identity drift");
  }
  if (metadata.scale_sha256 !== HOST_V2_RUNTIME_SOURCE_HASHES.metadataScale) {
    throw new TypeError("Commons SCALE metadata hash drift");
  }
  const fields = ["path", "scale_bytes", "scale_sha256", "runtime_metadata_version", "runtime_rfc78_hash"] as const;
  const expected = {
    path: metadata.scale_path,
    scale_bytes: metadata.scale_bytes,
    scale_sha256: metadata.scale_sha256,
    runtime_metadata_version: metadata.runtime_metadata_version,
    runtime_rfc78_hash: metadata.runtime_rfc78_hash,
  };
  for (const field of fields) {
    if (papi.metadata[field] !== expected[field]) {
      throw new TypeError(`generated PAPI metadata ${field} drift`);
    }
  }
  if (papi.schema !== "cord.commons-papi-descriptor.v1"
    || papi.entry !== "commons"
    || `${papi.generator.package}@${papi.generator.version}` !== metadata.generator) {
    throw new TypeError("generated PAPI descriptor identity drift");
  }
  if (network.genesis_hash !== metadata.genesis_identity
    || network.spec_version !== metadata.spec_version
    || network.transaction_version !== metadata.transaction_version
    || network.metadata_hash !== metadata.runtime_rfc78_hash) {
    throw new TypeError("Commons network and generated metadata binding drift");
  }
  return {
    runtime: metadata.runtime,
    specName: metadata.spec_name,
    genesisHash: metadata.genesis_identity,
    specVersion: metadata.spec_version,
    transactionVersion: metadata.transaction_version,
    metadataVersion: metadata.runtime_metadata_version,
    metadataRfc78Hash: metadata.runtime_rfc78_hash,
    metadataScale: {
      path: metadata.scale_path,
      bytes: metadata.scale_bytes,
      sha256: metadata.scale_sha256,
    },
    generatedDescriptor: {
      schema: papi.schema,
      entry: papi.entry,
      generator: metadata.generator,
      manifestSha256: HOST_V2_RUNTIME_SOURCE_HASHES.papiManifest,
    },
    network: {
      descriptorContractSha256: network.descriptor_contract_sha256,
      chainSpecSourceSha256: network.chain_spec_source_sha256,
      activationState: network.activation_state,
      productionActivationReady: network.production_activation_ready,
    },
  } as const;
}

export function projectPrivateHostV2Descriptor(
  operations: FrozenHostV2Operations,
  errors: FrozenHostV2Errors,
  metadata: FrozenCommonsMetadata,
  papi: FrozenPapiManifest,
  network: FrozenCommonsNetworkBinding,
) {
  assertFrozenOperations(operations);
  if (errors.errors.length !== 91) throw new TypeError(`host v2 error count drift: ${errors.errors.length}`);
  const errorCodes = errors.errors.map(({ code }) => code);
  if (new Set(errorCodes).size !== errorCodes.length) throw new TypeError("duplicate host v2 error code");
  const projected = operations.operations.map(projectOperation);
  const storage = projected.filter(({ code }) => code >= 1000 && code <= 1061);
  const identity = projected.filter(({ code }) => code >= 1100 && code <= 1106);
  const transaction = projected.filter(({ code }) => code === 1200);
  if (storage.length !== 26 || identity.length !== 7 || transaction.length !== 1) {
    throw new TypeError("host v2 domain projection count drift");
  }
  const features = Object.fromEntries([...new Set(projected.map(({ feature }) => feature))]
    .sort()
    .map((feature) => [feature, projected.filter((operation) => operation.feature === feature)
      .map(({ code }) => code)]));
  const featureNames = Object.keys(features).sort();
  if (featureNames.length !== HOST_V2_REQUIRED_NEGOTIATION_FEATURES.length
    || featureNames.some((feature, index) => feature !== HOST_V2_REQUIRED_NEGOTIATION_FEATURES[index])) {
    throw new TypeError("host v2 required negotiation feature drift");
  }
  const errorFamilies = Object.fromEntries([...new Set(errors.errors.map(({ family }) => family))]
    .sort()
    .map((family) => [family, errors.errors.filter((error) => error.family === family)
      .map(({ code }) => code)]));
  return {
    schema: "cord.origin.host-private-descriptor.v2",
    protocol: { id: HOST_V2_PROTOCOL, major: HOST_V2_MAJOR, minor: HOST_V2_MINOR },
    visibility: { public: false, testOnly: true, cutover: "P4/P5" },
    sources: HOST_V2_SOURCE_HASHES,
    runtimeSources: HOST_V2_RUNTIME_SOURCE_HASHES,
    runtimeBinding: projectRuntimeBinding(metadata, papi, network),
    operationCount: projected.length,
    errorCount: errors.errors.length,
    requiredNegotiationFeatures: HOST_V2_REQUIRED_NEGOTIATION_FEATURES,
    features,
    surfaces: {
      storage,
      identity: { composite: false, operations: identity },
      transaction: { separateSigning: true, operations: transaction },
    },
    errors: { encoding: errors.encoding, families: errorFamilies },
    legacyPublic: { unchanged: true, hashes: LEGACY_PUBLIC_DESCRIPTOR_HASHES },
  } as const;
}
