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
import { readFile } from "node:fs/promises";
import test from "node:test";

import {
  ATTESTATION_RESOLUTION_POLICY, NAMES_EVENT_KINDS, deriveNameId, deriveRegistrationCommitment,
  normalizedLabel, registrationSalt,
} from "../../src/names.ts";
import type { AccountId, BlockHash, NameId } from "../../src/types.ts";

interface Vectors {
  readonly label_policy_version: number;
  readonly attestation_resolution_policy: string;
  readonly valid_labels: readonly string[];
  readonly invalid_labels: readonly string[];
  readonly derivations: readonly { genesis_hash: string; parent: string | null; label: string; name_id: string }[];
  readonly commitments: readonly { genesis_hash: string; owner: string; parent: string | null; label: string; salt: string; name_id: string; commitment: string }[];
  readonly event_kinds: readonly string[];
}
const vectors = JSON.parse(await readFile(new URL("../../../docs/sdk/vectors/names-v1.json", import.meta.url), "utf8")) as Vectors;

test("shared Orbis Names canonical vectors match TypeScript SDK", () => {
  assert.equal(vectors.label_policy_version, 1);
  assert.equal(vectors.attestation_resolution_policy, ATTESTATION_RESOLUTION_POLICY);
  for (const label of vectors.valid_labels) assert.equal(normalizedLabel(label), label);
  for (const label of vectors.invalid_labels) assert.throws(() => normalizedLabel(label));
  for (const vector of vectors.derivations) {
    assert.equal(deriveNameId(vector.genesis_hash as BlockHash, vector.parent as NameId | null, normalizedLabel(vector.label)), vector.name_id);
  }
  for (const vector of vectors.commitments) {
    const name = deriveNameId(vector.genesis_hash as BlockHash, vector.parent as NameId | null, normalizedLabel(vector.label));
    assert.equal(name, vector.name_id);
    assert.equal(deriveRegistrationCommitment(vector.genesis_hash as BlockHash, vector.owner as AccountId, vector.parent as NameId | null, normalizedLabel(vector.label), registrationSalt(vector.salt)), vector.commitment);
  }
  assert.deepEqual(NAMES_EVENT_KINDS, vectors.event_kinds);
});
