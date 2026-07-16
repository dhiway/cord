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

import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import test from 'node:test';

type Vector = Record<string, unknown> & {
  id: string;
  canonical_cbor_hex: string;
  canonical_sha256: string;
};

type CheckpointVectors = {
  version: number;
  type: string;
  positive: Vector & {
    signed_message_hex: string;
    digest_hex: string;
  };
  negative: Array<Vector & {
    expected_error_code: number;
    pre_state_sha256: string;
    post_state_sha256: string;
    effect_count: number;
    event_count: number;
  }>;
};

const root = new URL('../../../', import.meta.url);
const vectors = JSON.parse(
  readFileSync(new URL('docs/specs/checkpoint-v2.vectors.json', root), 'utf8'),
) as CheckpointVectors;

function sha256(hex: string): string {
  return createHash('sha256').update(Buffer.from(hex, 'hex')).digest('hex');
}

function assertFrozenHashes(vector: Vector): void {
  assert.match(vector.canonical_cbor_hex, /^[0-9a-f]+$/);
  assert.match(vector.canonical_sha256, /^[0-9a-f]{64}$/);
  assert.equal(sha256(vector.canonical_cbor_hex), vector.canonical_sha256, `${vector.id}:canonical_cbor_hex`);
  for (const [name, value] of Object.entries(vector)) {
    if (!name.endsWith('_hex') || typeof value !== 'string') continue;
    const digest = vector[name.replace(/_hex$/, '_sha256')];
    if (typeof digest === 'string') assert.equal(sha256(value), digest, `${vector.id}:${name}`);
  }
}

test('checkpoint proof vectors freeze rejects, evidence, and suspension', () => {
  assert.equal(vectors.version, 2);
  assert.equal(vectors.type, 'CheckpointSubmissionV2');
  assertFrozenHashes(vectors.positive);
  assert.equal(sha256(String(vectors.positive.exact_response_cbor_hex)), vectors.positive.exact_response_sha256);
  assert.equal(sha256(String(vectors.positive.exact_event_cbor_hex)), vectors.positive.exact_event_sha256);
  assert.match(vectors.positive.signed_message_hex, /^[0-9a-f]+$/);
  assert.match(vectors.positive.digest_hex, /^[0-9a-f]{64}$/);

  for (const vector of vectors.negative) {
    assertFrozenHashes(vector);
    assert.equal(sha256(String(vector.exact_response_cbor_hex)), vector.exact_response_sha256);
  }

  const frozenCodes = new Map([
    ['checkpoint-wrong-domain', 220],
    ['checkpoint-wrong-version', 221],
    ['checkpoint-wrong-bucket', 222],
    ['checkpoint-wrong-key', 223],
    ['checkpoint-stale-nonce', 224],
    ['checkpoint-wrong-window', 225],
  ]);
  for (const [id, code] of frozenCodes) {
    const vector = vectors.negative.find((candidate) => candidate.id === id);
    assert.ok(vector, `missing ${id}`);
    assert.equal(vector.expected_error_code, code);
    assert.equal(vector.pre_state_sha256, vector.post_state_sha256);
    assert.equal(vector.effect_count, 0);
    assert.equal(vector.event_count, 0);
  }

  const equivocation = vectors.negative.find((vector) => vector.id === 'checkpoint-equivocation');
  assert.ok(equivocation);
  assert.equal(equivocation.effect_count, 2);
  assert.equal(equivocation.event_count, 1);
  assert.notEqual(equivocation.pre_state_sha256, equivocation.post_state_sha256);
});
