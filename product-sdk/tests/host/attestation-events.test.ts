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
import test from "node:test";

import {
  subscribeAttestationEvents,
  type TypedFinalizedEventBlock,
  type TypedFinalizedEventSource,
} from "../../packages/host/src/attestation-events.ts";
import { attestationEventSubscription } from "@cord-network/origin-sdk-attestation";
import type { BlockHash } from "../../src/types.ts";

const ANCHOR = `0x${"11".repeat(32)}` as BlockHash;
const FINALIZED = `0x${"22".repeat(32)}`;
const ATTESTATION = `0x${"33".repeat(32)}`;
const SCHEMA = `0x${"44".repeat(32)}`;
const SUBJECT = `0x${"55".repeat(32)}`;
const EXTERNAL_KEY = `0x${"66".repeat(32)}`;

class FakeFinalizedEventSource implements TypedFinalizedEventSource {
  observedAnchor?: string;

  async *subscribeFinalizedEvents(options: {
    readonly from: string;
    readonly signal: AbortSignal;
  }): AsyncIterable<TypedFinalizedEventBlock> {
    this.observedAnchor = options.from;
    yield {
      hash: FINALIZED,
      events: [
        { pallet: "Balances", event: "Transfer", data: {}, index: 0 },
        {
          pallet: "Attestation",
          event: "AttestationIssued",
          index: 1,
          data: {
            attestation: ATTESTATION,
            schema: SCHEMA,
            issuer: "issuer:alice",
            subject_commitment: SUBJECT,
          },
        },
        {
          pallet: "Attestation",
          event: "ExternalStatusRevoked",
          index: 2,
          data: {
            key: EXTERNAL_KEY,
            issuer: "issuer:alice",
            status_commitment: `0x${"77".repeat(32)}`,
            revoked_at: 42,
          },
        },
        {
          pallet: "Attestation",
          event: "AttestationRevoked",
          index: 3,
          data: { attestation: ATTESTATION, by: "issuer:alice", forced: false },
        },
      ],
    };
  }
}

test("finalized Host stream decodes and filters native Attestation events", async () => {
  const source = new FakeFinalizedEventSource();
  const subscription = attestationEventSubscription(ANCHOR, [
    "attestation_issued",
    "attestation_revoked",
  ]);
  const received = [];
  for await (const item of subscribeAttestationEvents(source, subscription)) received.push(item);

  assert.equal(source.observedAnchor, ANCHOR);
  assert.deepEqual(received, [
    {
      event: {
        finalized_block_hash: FINALIZED,
        event_index: 1,
        event: {
          event: "attestation_issued",
          data: {
            attestation: ATTESTATION,
            schema: SCHEMA,
            issuer: "issuer:alice",
            subject_commitment: SUBJECT,
          },
        },
      },
      outcome: { outcome: "attestation_available", data: { attestation: ATTESTATION } },
    },
    {
      event: {
        finalized_block_hash: FINALIZED,
        event_index: 3,
        event: {
          event: "attestation_revoked",
          data: { attestation: ATTESTATION, by: "issuer:alice", forced: false },
        },
      },
      outcome: { outcome: "attestation_revoked", data: { attestation: ATTESTATION } },
    },
  ]);
});

test("finalized Host stream decodes every native Attestation event variant", async () => {
  const events = [
    {
      pallet: "Attestation",
      event: "SchemaCreated",
      index: 0,
      data: {
        schema: SCHEMA,
        creator: "issuer:alice",
        definition_commitment: `0x${"88".repeat(32)}`,
        revocable: true,
        unique: false,
        index_policy: "issuer_and_subject_schema",
      },
    },
    {
      pallet: "Attestation",
      event: "SchemaStatusChanged",
      index: 1,
      data: { schema: SCHEMA, status: "paused", forced: false },
    },
    {
      pallet: "Attestation",
      event: "AttestationIssued",
      index: 2,
      data: {
        attestation: ATTESTATION,
        schema: SCHEMA,
        issuer: "issuer:alice",
        subject_commitment: SUBJECT,
      },
    },
    {
      pallet: "Attestation",
      event: "DelegatedIntentConsumed",
      index: 3,
      data: {
        issuer: "issuer:alice",
        delegate: "delegate:bob",
        nonce: 7n,
        attestation: ATTESTATION,
      },
    },
    {
      pallet: "Attestation",
      event: "DelegatedRevocationConsumed",
      index: 4,
      data: {
        revoker: "issuer:alice",
        delegate: "delegate:bob",
        nonce: 8n,
        attestation: ATTESTATION,
      },
    },
    {
      pallet: "Attestation",
      event: "AttestationRevoked",
      index: 5,
      data: { attestation: ATTESTATION, by: null, forced: true },
    },
    {
      pallet: "Attestation",
      event: "ExternalStatusRevoked",
      index: 6,
      data: {
        key: EXTERNAL_KEY,
        issuer: "issuer:alice",
        status_commitment: `0x${"77".repeat(32)}`,
        revoked_at: 42,
      },
    },
    {
      pallet: "Attestation",
      event: "EmergencyPauseChanged",
      index: 7,
      data: { paused: true },
    },
  ] as const;
  const source: TypedFinalizedEventSource = {
    async *subscribeFinalizedEvents() {
      yield { hash: FINALIZED, events };
    },
  };
  const kinds = [
    "schema_created",
    "schema_status_changed",
    "attestation_issued",
    "delegated_intent_consumed",
    "delegated_revocation_consumed",
    "attestation_revoked",
    "external_status_revoked",
    "emergency_pause_changed",
  ] as const;
  const received = [];
  for await (const item of subscribeAttestationEvents(
    source,
    attestationEventSubscription(ANCHOR, kinds),
  )) received.push([item.event.event.event, item.outcome.outcome]);

  assert.deepEqual(received, [
    ["schema_created", "schema_available"],
    ["schema_status_changed", "schema_status_changed"],
    ["attestation_issued", "attestation_available"],
    ["delegated_intent_consumed", "delegation_consumed"],
    ["delegated_revocation_consumed", "delegation_consumed"],
    ["attestation_revoked", "attestation_revoked"],
    ["external_status_revoked", "external_status_revoked"],
    ["emergency_pause_changed", "emergency_pause_changed"],
  ]);
});
