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

import { decodeHostV2, encodeHostV2, HostV2CodecError } from "../src/internal/v2/codec.ts";
import {
  HOST_V2_OPERATION_BINDINGS,
  HOST_V2_PROTOCOL,
  HOST_V2_REGISTRY_SHA256,
  type AcceptedEventV2,
} from "../src/internal/v2/generated.ts";
import { HostV2Session, HostV2SessionError } from "../src/internal/v2/session.ts";

const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), "../../../..");
const fixture = JSON.parse(readFileSync(
  resolve(repositoryRoot, "docs/specs/origin-host-registry-v2.vectors.json"),
  "utf8",
)) as { vectors: Array<{ id: string; wire_hex: string }> };

function vector(id: string): Uint8Array {
  const wire = fixture.vectors.find((candidate) => candidate.id === id)?.wire_hex;
  assert.ok(wire, `missing frozen host-v2 vector ${id}`);
  return Uint8Array.from(Buffer.from(wire, "hex"));
}

test("generated private host-v2 bindings remain in drift check", () => {
  const result = spawnSync(process.execPath, [
    "product-sdk/tools/origin-host-conformance/generate-origin-host-types.cjs",
    "--check",
  ], { cwd: repositoryRoot, encoding: "utf8" });
  assert.equal(result.status, 0, `${result.stdout}\n${result.stderr}`);
  assert.equal(HOST_V2_PROTOCOL, "cord.origin.host/2");
  assert.equal(HOST_V2_REGISTRY_SHA256, fixtureRegistryHash());
  assert.equal(Object.keys(HOST_V2_OPERATION_BINDINGS).length, 34);
  const binding = HOST_V2_OPERATION_BINDINGS["storage.bucket.get"];
  assert.equal(binding.code, 1001);
  assert.equal(binding.featureId, "storage.control");
  assert.equal(binding.grantScope, "storage.bucket.read");
  assert.equal(binding.consentMode, "grant");
  assert.equal(binding.stateChanging, false);
  assert.equal(binding.operationIdRequired, false);
  assert.equal(binding.frame, "StorageBucketGetFrame");
  assert.equal(binding.request, "StorageBucketGetRequest");
  assert.equal(binding.result, "StorageBucketGetResult");
  assert.deepEqual(binding.allowedErrors.slice(0, 3), [100, 101, 102]);
});

function fixtureRegistryHash(): string {
  const parsed = JSON.parse(readFileSync(
    resolve(repositoryRoot, "docs/specs/origin-host-registry-v2.vectors.json"),
    "utf8",
  )) as { registry_sha256: string };
  return parsed.registry_sha256;
}

test("private codec round-trips the frozen request and rejects non-canonical order", () => {
  const canonical = vector("wire-1001-canonical");
  const decoded = decodeHostV2("StorageBucketGetFrame", canonical);
  assert.equal(decoded.value[0], 2);
  assert.equal(decoded.value[3], 1001);
  assert.deepEqual(encodeHostV2(decoded.production, decoded.value), canonical);
  for (const id of [
    "wire-noncanonical-long-version",
    "wire-noncanonical-indefinite-map",
    "wire-noncanonical-reversed-map",
    "wire-noncanonical-tag",
  ]) {
    assert.throws(
      () => decodeHostV2("StorageBucketGetFrame", vector(id)),
      (error) => error instanceof HostV2CodecError && error.code === "WIRE_NON_CANONICAL",
      id,
    );
  }
});

test("private session enforces accepted-first, exact sequencing, and terminal state", () => {
  const requestId = new Uint8Array(16).fill(0x11);
  const accepted: AcceptedEventV2 = { 0: 2, 1: requestId, 2: 0, 3: 0, 4: { 0: 0 } };
  const acceptedWire = encodeHostV2("AcceptedEventV2", accepted);
  const errorWire = vector("error-100-wire_schema_invalid");
  const session = new HostV2Session(requestId);

  assert.equal(session.accept(acceptedWire)[3], 0);
  assert.equal(session.accept(errorWire)[3], 3);
  assert.equal(session.isTerminal, true);
  assert.throws(() => session.accept(errorWire), HostV2SessionError);
  assert.throws(() => new HostV2Session(requestId).accept(errorWire), HostV2SessionError);
});
