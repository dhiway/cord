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

const root = new URL('../../../', import.meta.url);

function json(path: string): Record<string, any> {
  return JSON.parse(readFileSync(new URL(path, root), 'utf8'));
}

function sha256(bytes: Uint8Array): string {
  return createHash('sha256').update(bytes).digest('hex');
}

function assertFrozenHashes(vector: Record<string, any>): void {
  for (const [name, encoded] of Object.entries(vector)) {
    if (!name.endsWith('_cbor_hex') || typeof encoded !== 'string') continue;
    const digestName = `${name.slice(0, -'_cbor_hex'.length)}_sha256`;
    if (!(digestName in vector)) continue;
    assert.equal(sha256(Buffer.from(encoded, 'hex')), vector[digestName], `${vector.id}:${name}`);
  }
}

test('host v2 private outbox preserves exact loss and duplicate-effect boundaries', () => {
  const cddl = readFileSync(new URL('docs/specs/origin-host-registry-v2.cddl', root));
  const hostVectors = json('docs/specs/origin-host-registry-v2.vectors.json');
  const protocolVectors = json('docs/specs/protocol-executable-v2.vectors.json');
  const stateMachine = json('docs/specs/host-outbox-v1.state-machine.json');
  const outbox = json('docs/specs/host-outbox-v1.vectors.json');

  assert.equal(sha256(cddl), hostVectors.registry_sha256);
  assert.equal(stateMachine.authority, 'host-provider-protocol-v2.md#durable-pre-send-outbox');
  assert.deepEqual(stateMachine.states, [
    'Prepared', 'SentAdvisory', 'ResponseInstalled', 'AckConfirmed', 'Expired', 'Quarantined', 'GC',
  ]);
  assert.ok(stateMachine.transitions.some((row: any) =>
    row.from === 'SentAdvisory'
    && row.event === 'transport_loss'
    && row.to === 'Prepared'
    && row.effect === 'resend_byte_identical_record'));

  assertFrozenHashes(outbox.base_vector);
  for (const vector of outbox.crash_vectors) assertFrozenHashes(vector);
  assert.equal(new Set(outbox.crash_vectors.map((row: any) => row.id)).size, outbox.crash_vectors.length);
  assert.equal(outbox.crash_vectors.reduce(
    (duplicates: number, row: any) => duplicates + Math.max(0, row.effect_count - 1), 0,
  ), 0);

  const expectedEffects = new Map([
    ['failure-before-persist', 0], ['after-persist-before-send', 0],
    ['after-send-before-provider-commit', 0], ['provider-commit-before-accepted', 1],
    ['accepted-before-host-install', 1], ['host-install-before-ack', 1],
    ['ack-confirmation-loss', 1], ['ack-confirmed-before-gc', 1],
    ['cancel-before-provider-commit', 1], ['cancel-after-provider-commit', 1],
    ['host-restart', 0], ['provider-restart', 1], ['joint-restart', 0],
    ['keystore-unavailable', 0], ['ciphertext-corrupt', 0], ['expiry-gc-race', 0],
  ]);
  assert.equal(expectedEffects.size, outbox.crash_vectors.length);
  for (const vector of outbox.crash_vectors) assert.equal(vector.effect_count, expectedEffects.get(vector.id));

  const protocolOutbox = protocolVectors.vectors.find((row: any) => row.id === 'host-outbox-entry-v1');
  assert.ok(protocolOutbox);
  assert.equal(protocolOutbox.canonical_sha256, outbox.base_vector.canonical_sha256);
  assert.equal(protocolOutbox.effect_count, outbox.base_vector.effect_count);
});
