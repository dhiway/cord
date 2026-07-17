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
import { readFileSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";

import {
  decodeHostV2,
  encodeHostV2,
  encodeHostV2Value,
  HostV2CodecError,
} from "../src/internal/v2/codec.ts";
import {
  HOST_V2_ERROR_BINDINGS,
  HOST_V2_FEATURE_IDS,
  HOST_V2_MAJOR,
  HOST_V2_MINOR,
  HOST_V2_OPERATION_BINDINGS,
  HOST_V2_PROTOCOL,
  HOST_V2_REGISTRY_SHA256,
  HOST_V2_SCHEMAS,
  type AcceptedEventV2,
  type HostV2FeatureId,
  type HostV2TypeName,
  type ProgressEventV2,
} from "../src/internal/v2/generated.ts";
import {
  HostV2NegotiationError,
  HostV2Session,
  HostV2SessionError,
  negotiateHostV2,
  type HostV2NegotiationOffer,
  type HostV2Negotiated,
} from "../src/internal/v2/session.ts";

const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), "../../../..");
const fixture = JSON.parse(readFileSync(
  resolve(repositoryRoot, "docs/specs/origin-host-registry-v2.vectors.json"),
  "utf8",
)) as { registry_sha256: string; vectors: Array<{ id: string; wire_hex: string; code?: number }> };
const operationRegistry = JSON.parse(readFileSync(
  resolve(repositoryRoot, "docs/specs/origin-host-registry-v2.operations.json"),
  "utf8",
)) as {
  major: number;
  minor: number;
  operations: Array<{
    name: keyof typeof HOST_V2_OPERATION_BINDINGS;
    code: number;
    feature_id: string;
    grant_scope: string;
    consent_mode: string;
    state_changing: boolean;
    operation_id_required: boolean;
    cddl: { Request: string; Accepted: string; Progress: string; Result: string; Error: string };
    allowed_errors: Array<{ code: number }>;
    positive_vectors: string[];
    negative_vectors: string[];
  }>;
};
const errorRegistry = JSON.parse(readFileSync(
  resolve(repositoryRoot, "docs/specs/origin-host-registry-v2.errors.json"),
  "utf8",
)) as { errors: Array<{ code: number; name: string; retryable: boolean }> };

function vector(id: string): Uint8Array {
  const wire = fixture.vectors.find((candidate) => candidate.id === id)?.wire_hex;
  assert.ok(wire, `missing frozen host-v2 vector ${id}`);
  return Uint8Array.from(Buffer.from(wire, "hex"));
}

function offer(overrides: Partial<HostV2NegotiationOffer> = {}): HostV2NegotiationOffer {
  return {
    protocol: HOST_V2_PROTOCOL,
    major: HOST_V2_MAJOR,
    minors: [HOST_V2_MINOR],
    genesis: new Uint8Array(32).fill(0x42),
    finalizedSpecVersion: 31,
    finalizedTransactionVersion: 8,
    registrySha256: HOST_V2_REGISTRY_SHA256,
    features: [...HOST_V2_FEATURE_IDS],
    ...overrides,
  };
}

function negotiated() {
  return negotiateHostV2(offer(), offer());
}

function accepted(requestId: Uint8Array, sequence = 0): Uint8Array {
  return encodeHostV2("AcceptedEventV2", {
    0: 2,
    1: requestId,
    2: sequence,
    3: 0,
    4: { 0: 0 },
  } as AcceptedEventV2);
}

function progress(requestId: Uint8Array, sequence: number): Uint8Array {
  return encodeHostV2("ProgressEventV2", {
    0: 2,
    1: requestId,
    2: sequence,
    3: 1,
    4: { 0: 1 },
  } as ProgressEventV2);
}

test("generated runtime bindings exactly project all frozen authorities", () => {
  const result = spawnSync(process.execPath, [
    "product-sdk/tools/origin-host-conformance/generate-origin-host-types.cjs",
    "--check",
  ], { cwd: repositoryRoot, encoding: "utf8" });
  assert.equal(result.status, 0, `${result.stdout}\n${result.stderr}`);
  assert.equal(HOST_V2_PROTOCOL, "cord.origin.host/2");
  assert.equal(HOST_V2_MAJOR, operationRegistry.major);
  assert.equal(HOST_V2_MINOR, operationRegistry.minor);
  assert.equal(HOST_V2_REGISTRY_SHA256, fixture.registry_sha256);
  assert.equal(Object.keys(HOST_V2_SCHEMAS).length, 365);
  assert.equal(Object.keys(HOST_V2_OPERATION_BINDINGS).length, 34);
  assert.equal(Object.keys(HOST_V2_ERROR_BINDINGS).length, 91);

  for (const operation of operationRegistry.operations) {
    const binding = HOST_V2_OPERATION_BINDINGS[operation.name];
    assert.deepEqual(binding, {
      code: operation.code,
      featureId: operation.feature_id,
      grantScope: operation.grant_scope,
      consentMode: operation.consent_mode,
      stateChanging: operation.state_changing,
      operationIdRequired: operation.operation_id_required,
      frame: operation.cddl.Request.replace(/Request$/, "Frame"),
      request: operation.cddl.Request,
      accepted: operation.cddl.Accepted,
      progress: operation.cddl.Progress,
      result: operation.cddl.Result,
      error: operation.cddl.Error,
      allowedErrors: operation.allowed_errors.map(({ code }) => code),
    }, operation.name);
  }
  for (const error of errorRegistry.errors) {
    assert.deepEqual(HOST_V2_ERROR_BINDINGS[error.code as keyof typeof HOST_V2_ERROR_BINDINGS], {
      name: error.name,
      retryable: error.retryable,
    }, error.name);
  }
});

test("all 34 frozen operation frames round-trip and their schema negatives fail", () => {
  for (const operation of operationRegistry.operations) {
    const production = HOST_V2_OPERATION_BINDINGS[operation.name].frame as HostV2TypeName;
    for (const id of operation.positive_vectors) {
      const canonical = vector(id);
      const decoded = decodeHostV2(production, canonical);
      assert.deepEqual(encodeHostV2(production, decoded.value), canonical, id);
    }
    for (const id of operation.negative_vectors.filter((candidate) => candidate.endsWith("schema-negative"))) {
      assert.throws(() => decodeHostV2(production, vector(id)), HostV2CodecError, id);
    }
  }
});

test("all 91 frozen error events round-trip through the closed error union", () => {
  const errorVectors = fixture.vectors.filter(({ id }) => id.startsWith("error-"));
  assert.equal(errorVectors.length, 91);
  for (const { id } of errorVectors) {
    const canonical = vector(id);
    const decoded = decodeHostV2("EventV2", canonical);
    assert.deepEqual(encodeHostV2("EventV2", decoded.value), canonical, id);
  }
});

test("canonical codec rejects every hostile wire class, unknown fields/errors, and invalid IDs", () => {
  const nonCanonical = [
    "wire-noncanonical-long-version",
    "wire-noncanonical-indefinite-map",
    "wire-noncanonical-reversed-map",
    "wire-noncanonical-tag",
    "wire-noncanonical-indefinite-bytes",
  ];
  for (const id of nonCanonical) {
    assert.throws(
      () => decodeHostV2("StorageBucketGetFrame", vector(id)),
      (error) => error instanceof HostV2CodecError && error.code === "WIRE_NON_CANONICAL",
      id,
    );
  }
  for (const id of ["wire-schema-duplicate-key", "wire-schema-float"]) {
    assert.throws(() => decodeHostV2("StorageBucketGetFrame", vector(id)), HostV2CodecError, id);
  }
  assert.throws(() => decodeHostV2("RequestId", vector("wire-schema-invalid-utf8")), HostV2CodecError);
  assert.throws(() => encodeHostV2("RequestId", new Uint8Array(15)), /byte bounds/);
  assert.throws(() => encodeHostV2("OperationId", new Uint8Array(17)), /byte bounds/);
  assert.throws(() => encodeHostV2("Nonce", new Uint8Array(0)), /byte bounds/);
  assert.throws(() => encodeHostV2("ProductId", "\ud800"), /unpaired UTF-16 surrogate/);

  const unknownField = encodeHostV2Value({ 0: 0, 1: 1 });
  assert.throws(() => decodeHostV2("AcceptedState", unknownField), /unknown field/);
  const unknownError = encodeHostV2Value({
    0: 2,
    1: new Uint8Array(16),
    2: 0,
    3: 3,
    4: { 0: 65535, 1: "UNKNOWN", 2: false, 3: {} },
  });
  assert.throws(() => decodeHostV2("EventV2", unknownError), /closed union/);
});

test("Drive manifest CID ordering is text-parity-safe and byte-stable", () => {
  const hash = new Uint8Array(32);
  const wire = encodeHostV2("DriveManifestV1", { 0: 1, 1: hash, 2: 0, 3: ["a", "b"], 4: hash });
  assert.equal(Buffer.from(wire).toString("hex"), "a50001015820000000000000000000000000000000000000000000000000000000000000000002000382616161620458200000000000000000000000000000000000000000000000000000000000000000");
  assert.deepEqual(encodeHostV2("DriveManifestV1", decodeHostV2("DriveManifestV1", wire).value), wire);
  assert.throws(() => encodeHostV2("DriveManifestV1", { 0: 1, 1: hash, 2: 0, 3: ["b", "a"], 4: hash }), /sorted-unique/);
  assert.throws(() => encodeHostV2("DriveManifestV1", { 0: 1, 1: hash, 2: 0, 3: ["a", "a"], 4: hash }), /sorted-unique/);
});

test("negotiation binds descriptor, genesis, finalized versions, highest minor, and exact features", () => {
  const local = offer({ features: ["storage.content", "storage.s3", "identity.account"] });
  const remote = offer({ features: ["identity.account", "storage.s3"] });
  const result = negotiateHostV2(local, remote);
  assert.equal(result.major, HOST_V2_MAJOR);
  assert.equal(result.minor, HOST_V2_MINOR);
  assert.equal(result.registrySha256, HOST_V2_REGISTRY_SHA256);
  assert.deepEqual(result.features, ["identity.account", "storage.s3"]);
  local.genesis.fill(0xff);
  assert.equal(result.genesis[0], 0x42, "negotiated tuple must snapshot genesis");

  assert.throws(() => negotiateHostV2(offer({ major: 3 }), offer()),
    (error) => error instanceof HostV2NegotiationError && error.code === "WIRE_VERSION_MISMATCH");
  assert.throws(() => negotiateHostV2(offer(), offer({ genesis: new Uint8Array(32).fill(1) })),
    (error) => error instanceof HostV2NegotiationError && error.code === "WIRE_GENESIS_MISMATCH");
  assert.throws(() => negotiateHostV2(offer({ registrySha256: "0".repeat(64) }), offer()),
    (error) => error instanceof HostV2NegotiationError && error.code === "WIRE_DESCRIPTOR_MISMATCH");
  assert.throws(() => negotiateHostV2(offer(), offer({ finalizedSpecVersion: 32 })),
    (error) => error instanceof HostV2NegotiationError && error.code === "WIRE_VERSION_MISMATCH");
  assert.throws(() => negotiateHostV2(offer({ features: ["storage.s3", "storage.s3"] }), offer()),
    (error) => error instanceof HostV2NegotiationError && error.code === "WIRE_DESCRIPTOR_MISMATCH");
  assert.throws(() => negotiateHostV2(offer({ features: ["unknown"] as HostV2FeatureId[] }), offer()),
    (error) => error instanceof HostV2NegotiationError && error.code === "WIRE_DESCRIPTOR_MISMATCH");
});

test("negotiated authority is opaque, immutable, and rejects brand or prototype forgery", () => {
  const authority = negotiated();
  const exposedGenesis = authority.genesis;
  exposedGenesis.fill(0xff);
  assert.equal(authority.genesis[0], 0x42, "genesis access must return a defensive copy");
  assert.ok(Object.isFrozen(authority.features));
  assert.throws(() => (authority.features as HostV2FeatureId[]).push("storage.s3"));
  assert.throws(() => Object.assign(authority, { minor: 65_535 }));
  assert.equal(authority.minor, HOST_V2_MINOR);

  const requestId = new Uint8Array(16) as Parameters<typeof accepted>[0];
  const plainObject = {
    protocol: authority.protocol,
    major: authority.major,
    minor: authority.minor,
    genesis: authority.genesis,
    finalizedSpecVersion: authority.finalizedSpecVersion,
    finalizedTransactionVersion: authority.finalizedTransactionVersion,
    registrySha256: authority.registrySha256,
    features: authority.features,
  } as unknown as HostV2Negotiated;
  assert.throws(
    () => new HostV2Session(plainObject, requestId),
    (error) => error instanceof HostV2NegotiationError && error.code === "WIRE_DESCRIPTOR_MISMATCH",
  );

  const prototypeForgery = Object.create(Object.getPrototypeOf(authority)) as HostV2Negotiated;
  assert.throws(
    () => new HostV2Session(prototypeForgery, requestId),
    (error) => error instanceof HostV2NegotiationError && error.code === "WIRE_DESCRIPTOR_MISMATCH",
  );

  const NegotiatedConstructor = authority.constructor as new (...args: unknown[]) => HostV2Negotiated;
  assert.throws(
    () => new NegotiatedConstructor(
      Symbol("forged"),
      authority.minor,
      authority.genesis,
      authority.finalizedSpecVersion,
      authority.finalizedTransactionVersion,
      authority.features,
    ),
    (error) => error instanceof HostV2NegotiationError && error.code === "WIRE_DESCRIPTOR_MISMATCH",
  );

  const constructorObject = authority.constructor as Function & { snapshot?: () => HostV2Negotiated };
  const prototype = Object.getPrototypeOf(authority) as object;
  assert.ok(Object.isFrozen(constructorObject));
  assert.ok(Object.isFrozen(prototype));
  assert.throws(() => Object.assign(constructorObject, { snapshot: () => plainObject }));
  assert.throws(() => Object.defineProperty(prototype, "minor", { get: () => 65_535 }));
  assert.throws(() => Object.setPrototypeOf(authority, { minor: 65_535 }));

  const originalWeakMapGet = WeakMap.prototype.get;
  try {
    WeakMap.prototype.get = function poisonedWeakMapGet() {
      return {
        minor: 65_535,
        genesis: new Uint8Array(32).fill(0xff),
        finalizedSpecVersion: 0,
        finalizedTransactionVersion: 0,
        features: [],
      } as never;
    };
    assert.throws(
      () => new HostV2Session(plainObject, requestId),
      (error) => error instanceof HostV2NegotiationError && error.code === "WIRE_DESCRIPTOR_MISMATCH",
    );
  } finally {
    WeakMap.prototype.get = originalWeakMapGet;
  }

  const session = new HostV2Session(authority, requestId);
  const sessionGenesis = session.negotiation.genesis;
  sessionGenesis.fill(0xaa);
  assert.equal(session.negotiation.genesis[0], 0x42);
});

test("session snapshots its ID, enforces sequence law, and permanently closes on every fault or terminal", () => {
  const requestId = new Uint8Array(16).fill(0x11);
  const session = new HostV2Session(negotiated(), requestId);
  requestId.fill(0x22);
  assert.equal(session.accept(accepted(new Uint8Array(16).fill(0x11)))[3], 0);
  assert.equal(session.accept(vector("error-100-wire_schema_invalid"))[3], 3);
  assert.equal(session.isTerminal, true);
  assert.equal(session.isClosed, true);
  assert.throws(() => session.accept(vector("error-100-wire_schema_invalid")), HostV2SessionError);

  const skipped = new HostV2Session(negotiated(), new Uint8Array(16).fill(0x11));
  skipped.accept(accepted(new Uint8Array(16).fill(0x11)));
  assert.throws(() => skipped.accept(progress(new Uint8Array(16).fill(0x11), 2)), HostV2SessionError);
  assert.equal(skipped.isClosed, true);
  assert.throws(() => skipped.accept(progress(new Uint8Array(16).fill(0x11), 1)), /permanently closed/);

  const duplicateAccepted = new HostV2Session(negotiated(), new Uint8Array(16).fill(0x11));
  duplicateAccepted.accept(accepted(new Uint8Array(16).fill(0x11)));
  assert.throws(() => duplicateAccepted.accept(accepted(new Uint8Array(16).fill(0x11), 1)), HostV2SessionError);
  assert.equal(duplicateAccepted.isClosed, true);

  const beforeAccepted = new HostV2Session(negotiated(), new Uint8Array(16).fill(0x11));
  assert.throws(() => beforeAccepted.accept(progress(new Uint8Array(16).fill(0x11), 0)), HostV2SessionError);
  assert.equal(beforeAccepted.isClosed, true);
  assert.throws(() => new HostV2Session(negotiated(), new Uint8Array(15) as never), HostV2CodecError);
});
