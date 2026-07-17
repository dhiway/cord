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
import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import {
  decodeHostV2,
  encodeHostV2,
  encodeHostV2Value,
  type HostV2Map,
  type HostV2Value,
} from "../../packages/origin-sdk-host/src/internal/v2/codec.ts";
import {
  HOST_V2_FEATURE_IDS,
  HOST_V2_MAJOR,
  HOST_V2_MINOR,
  HOST_V2_PROTOCOL,
  HOST_V2_REGISTRY_SHA256,
  type HostV2FeatureId,
  type HostV2TypeName,
} from "../../packages/origin-sdk-host/src/internal/v2/generated.ts";
import {
  HostV2Session,
  negotiateHostV2,
  type HostV2NegotiationOffer,
} from "../../packages/origin-sdk-host/src/internal/v2/session.ts";

const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), "../../..");
const vectorPath = resolve(repositoryRoot, "product-sdk/examples/festival/host-v2-mobile-conformance-vectors.json");
const manifestPath = resolve(repositoryRoot, "product-sdk/examples/festival/host-v2-mobile-conformance.manifest.json");
const operationsPath = resolve(repositoryRoot, "docs/specs/origin-host-registry-v2.operations.json");
const frozenPath = resolve(repositoryRoot, "docs/specs/origin-host-registry-v2.vectors.json");

type TaggedValue =
  | { readonly uint: string }
  | { readonly bytes: string }
  | { readonly text: string }
  | { readonly bool: boolean }
  | { readonly array: readonly TaggedValue[] }
  | { readonly map: readonly (readonly [string, TaggedValue])[] };

interface ProjectionVector {
  readonly id: string;
  readonly category: "storage" | "identity" | "signing" | "grant" | "resume" | "lifecycle";
  readonly production: HostV2TypeName;
  readonly operation?: string;
  readonly code?: number;
  readonly frame?: string;
  readonly source_vector?: string;
  readonly phase?: string;
  readonly projection: TaggedValue;
  readonly canonical_cbor_hex: string;
  readonly canonical_sha256: string;
}

interface Fixture {
  readonly schema: string;
  readonly protocol: { readonly name: string; readonly major: number; readonly minor: number; readonly registry_sha256: string };
  readonly scope: Record<string, boolean | string>;
  readonly coverage: Record<string, unknown>;
  readonly vectors: readonly ProjectionVector[];
  readonly grant_contracts: readonly Record<string, unknown>[];
  readonly negotiation: {
    readonly local_features: readonly HostV2FeatureId[];
    readonly remote_features: readonly HostV2FeatureId[];
    readonly expected_features: readonly HostV2FeatureId[];
  };
  readonly capability_gates: readonly {
    readonly capability: string;
    readonly available: boolean;
    readonly requires: string;
    readonly expected_error: string;
  }[];
  readonly excluded_legacy_surfaces: readonly string[];
}

export class MobileDeviceCapabilityError extends Error {
  readonly code = "MOBILE_DEVICE_CAPABILITY_UNSUPPORTED" as const;
  readonly capability: string;
  readonly requires: string;

  constructor(capability: string, requires: string) {
    super(`mobile device does not provide ${requires} for ${capability}`);
    this.name = "MobileDeviceCapabilityError";
    this.capability = capability;
    this.requires = requires;
  }
}

export function requireMobileDeviceCapability(gate: {
  readonly capability: string;
  readonly available: boolean;
  readonly requires: string;
}): void {
  if (!gate.available) throw new MobileDeviceCapabilityError(gate.capability, gate.requires);
}

function parseJson<T>(path: string): T {
  return JSON.parse(readFileSync(path, "utf8")) as T;
}

function sha256(value: Uint8Array | string): string {
  return createHash("sha256").update(value).digest("hex");
}

function fromTagged(value: TaggedValue): HostV2Value {
  if ("uint" in value) return BigInt(value.uint);
  if ("bytes" in value) return Uint8Array.from(Buffer.from(value.bytes, "hex"));
  if ("text" in value) return value.text;
  if ("bool" in value) return value.bool;
  if ("array" in value) return value.array.map(fromTagged);
  const map: Record<number, HostV2Value> = {};
  for (const [key, item] of value.map) map[Number(key)] = fromTagged(item);
  return map;
}

function toTagged(value: HostV2Value): TaggedValue {
  if (typeof value === "boolean") return { bool: value };
  if (typeof value === "number" || typeof value === "bigint") return { uint: String(value) };
  if (typeof value === "string") return { text: value };
  if (value instanceof Uint8Array) return { bytes: Buffer.from(value).toString("hex") };
  if (Array.isArray(value)) return { array: value.map(toTagged) };
  const map = value as HostV2Map;
  return {
    map: Object.keys(map).map(Number).sort((left, right) => left - right)
      .map((key) => [String(key), toTagged(map[key]!)] as const),
  };
}

function offer(features: readonly HostV2FeatureId[]): HostV2NegotiationOffer {
  return {
    protocol: HOST_V2_PROTOCOL,
    major: HOST_V2_MAJOR,
    minors: [HOST_V2_MINOR],
    genesis: new Uint8Array(32).fill(0x42),
    finalizedSpecVersion: 31,
    finalizedTransactionVersion: 8,
    registrySha256: HOST_V2_REGISTRY_SHA256,
    features,
  };
}

function reencode(production: HostV2TypeName, value: HostV2Value): Uint8Array {
  return encodeHostV2(production, value as never);
}

export function validateFestivalMobileHostV2Conformance(): { readonly vectors: number; readonly operations: number } {
  const fixture = parseJson<Fixture>(vectorPath);
  const manifest = parseJson<{
    readonly visibility: string;
    readonly off_chain_projection_only: boolean;
    readonly production_mobile_rewrite: boolean;
    readonly vector_sha256: string;
    readonly source_sha256: Record<string, string>;
  }>(manifestPath);
  const operations = parseJson<{ readonly operations: readonly Record<string, unknown>[] }>(operationsPath);
  const frozen = parseJson<{ readonly vectors: readonly { readonly id: string; readonly wire_hex: string; readonly wire_sha256?: string }[] }>(frozenPath);

  assert.equal(fixture.schema, "cord.festival.host-v2-mobile-projection-v1");
  assert.deepEqual(fixture.protocol, {
    name: HOST_V2_PROTOCOL,
    major: HOST_V2_MAJOR,
    minor: HOST_V2_MINOR,
    registry_sha256: HOST_V2_REGISTRY_SHA256,
  });
  assert.deepEqual(fixture.scope, {
    visibility: "private-test-only",
    off_chain_projection_only: true,
    production_mobile_rewrite: false,
    swift_sources_modified: false,
    kotlin_sources_modified: false,
  });
  assert.equal(fixture.vectors.length, 39);

  for (const vector of fixture.vectors) {
    assert.match(vector.canonical_cbor_hex, /^(?:[0-9a-f]{2})+$/);
    const expected = Uint8Array.from(Buffer.from(vector.canonical_cbor_hex, "hex"));
    const projected = fromTagged(vector.projection);
    assert.deepEqual(encodeHostV2Value(projected), expected, `${vector.id}: tagged projection bytes`);
    assert.equal(sha256(expected), vector.canonical_sha256, `${vector.id}: SHA-256`);
    const decoded = decodeHostV2(vector.production, expected);
    assert.deepEqual(reencode(vector.production, decoded.value as HostV2Value), expected, `${vector.id}: production bytes`);
    assert.deepEqual(toTagged(decoded.value as HostV2Value), vector.projection, `${vector.id}: lossless projection`);
  }

  const requests = fixture.vectors.filter((vector) => vector.production === "RequestV2");
  assert.equal(requests.length, 34);
  assert.equal(requests.filter((vector) => vector.category === "storage").length, 26);
  assert.equal(requests.filter((vector) => vector.category === "identity").length, 7);
  assert.equal(requests.filter((vector) => vector.category === "signing").length, 1);
  for (const vector of requests) {
    const operation = operations.operations.find((item) => item.name === vector.operation);
    assert.ok(operation, `${vector.id}: registered operation`);
    assert.equal(operation.code, vector.code);
    assert.equal(`${String((operation.cddl as Record<string, string>).Request).replace(/Request$/, "Frame")}`, vector.frame);
    const source = frozen.vectors.find((item) => item.id === vector.source_vector);
    assert.ok(source, `${vector.id}: frozen source`);
    assert.equal(source.wire_hex, vector.canonical_cbor_hex);
    assert.equal(source.wire_sha256, vector.canonical_sha256);
  }

  assert.equal(fixture.grant_contracts.length, operations.operations.length);
  assert.deepEqual(fixture.grant_contracts, operations.operations.map((operation) => ({
    operation: operation.name,
    code: operation.code,
    grant_scope: operation.grant_scope,
    consent_mode: operation.consent_mode,
    operation_id_required: operation.operation_id_required,
  })));
  assert.equal(fixture.vectors.filter((vector) => vector.production === "ProviderCapabilityV1").length, 1);
  assert.equal(fixture.vectors.filter((vector) => vector.production === "ResumeTokenV1").length, 1);

  const negotiated = negotiateHostV2(offer(fixture.negotiation.local_features), offer(fixture.negotiation.remote_features));
  assert.deepEqual(negotiated.features, fixture.negotiation.expected_features);
  assert.deepEqual(fixture.negotiation.local_features, HOST_V2_FEATURE_IDS);

  const lifecycle = Object.fromEntries(fixture.vectors.filter((vector) => vector.category === "lifecycle")
    .map((vector) => [vector.phase, Uint8Array.from(Buffer.from(vector.canonical_cbor_hex, "hex"))]));
  const session = new HostV2Session(negotiated, new Uint8Array(16).fill(0x11));
  session.accept(lifecycle.accepted!);
  assert.equal(session.isTerminal, false);
  session.accept(lifecycle.progress!);
  assert.equal(session.isTerminal, false);
  session.accept(lifecycle.cancelled!);
  assert.equal(session.isTerminal, true);
  assert.equal(session.isClosed, true);

  for (const gate of fixture.capability_gates) {
    assert.equal(gate.expected_error, "MOBILE_DEVICE_CAPABILITY_UNSUPPORTED");
    assert.throws(
      () => requireMobileDeviceCapability(gate),
      (error) => error instanceof MobileDeviceCapabilityError && error.code === gate.expected_error,
    );
  }

  assert.deepEqual(fixture.excluded_legacy_surfaces, ["personhood", "PeopleLite", "preimage", "TransactionStorage"]);
  const searchable = JSON.stringify([fixture.vectors, fixture.grant_contracts]).toLowerCase();
  for (const excluded of fixture.excluded_legacy_surfaces) assert.equal(searchable.includes(excluded.toLowerCase()), false);

  assert.equal(manifest.visibility, "private-test-only");
  assert.equal(manifest.off_chain_projection_only, true);
  assert.equal(manifest.production_mobile_rewrite, false);
  assert.equal(manifest.vector_sha256, sha256(readFileSync(vectorPath)));
  assert.equal(manifest.source_sha256["origin-host-registry-v2.operations.json"], sha256(readFileSync(operationsPath)));
  assert.equal(manifest.source_sha256["origin-host-registry-v2.vectors.json"], sha256(readFileSync(frozenPath)));
  const publicPackage = parseJson<{ readonly exports: Record<string, unknown> }>(
    resolve(repositoryRoot, "product-sdk/packages/origin-sdk-host/package.json"),
  );
  assert.deepEqual(Object.keys(publicPackage.exports).sort(), [".", "./testing"]);
  assert.equal(readFileSync(resolve(repositoryRoot, "product-sdk/packages/origin-sdk-host/src/index.ts"), "utf8").includes("mobile-conformance"), false);

  return { vectors: fixture.vectors.length, operations: operations.operations.length };
}
