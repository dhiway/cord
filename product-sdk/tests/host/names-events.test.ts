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
import { decodeNamesEvent, subscribeNamesEvents, type FinalizedNamesOutcome } from "../../packages/host/src/names-events.ts";
import type { TypedFinalizedEvent, TypedFinalizedEventSource } from "../../packages/host/src/attestation-events.ts";
import { NAMES_EVENT_KINDS, namesEventSubscription } from "../../src/names.ts";
import type { BlockHash } from "../../src/types.ts";

const anchor = `0x${"11".repeat(32)}` as BlockHash;
const finalized = `0x${"22".repeat(32)}`;
const name = `0x${"33".repeat(32)}`;
const commitment = `0x${"44".repeat(32)}`;
const owner = "owner:alice";
const common = { name };
const events: readonly TypedFinalizedEvent[] = [
  { pallet: "Names", event: "CommitmentStored", index: 0, data: { owner, commitment, at: 1 } },
  { pallet: "Names", event: "CommitmentRemoved", index: 1, data: { owner, commitment } },
  { pallet: "Names", event: "NameRegistered", index: 2, data: { ...common, parent: null, label: { asBytes: () => new TextEncoder().encode("alice") }, owner, expires_at: 2 } },
  { pallet: "Names", event: "NameRenewed", index: 3, data: { ...common, expires_at: 3 } },
  { pallet: "Names", event: "NameTransferred", index: 4, data: { ...common, from: owner, to: "owner:bob" } },
  { pallet: "Names", event: "NameReleased", index: 5, data: { ...common, owner } },
  { pallet: "Names", event: "ExpiredNameRemoved", index: 6, data: common },
  { pallet: "Names", event: "ControllerAdded", index: 7, data: { ...common, controller: "controller:bob" } },
  { pallet: "Names", event: "ControllerRemoved", index: 8, data: { ...common, controller: "controller:bob" } },
  { pallet: "Names", event: "AddressSet", index: 9, data: { ...common, present: true } },
  { pallet: "Names", event: "SubjectSet", index: 10, data: { ...common, present: true } },
  { pallet: "Names", event: "AttestationSet", index: 11, data: { ...common, present: true } },
  { pallet: "Names", event: "ContentSet", index: 12, data: { ...common, present: true } },
  { pallet: "Names", event: "TextSet", index: 13, data: { ...common, key: new TextEncoder().encode("url"), present: true } },
  { pallet: "Names", event: "PrimaryNameSet", index: 14, data: { owner, name } },
  { pallet: "Names", event: "NameReserved", index: 15, data: { ...common, beneficiary: owner, expires_at: 4 } },
  { pallet: "Names", event: "ReservationCleared", index: 16, data: common },
  { pallet: "Names", event: "LabelProtectionSet", index: 17, data: { label: "0x726f6f74", protected: true } },
  { pallet: "Names", event: "PauseSet", index: 18, data: { paused: true } },
  { pallet: "Names", event: "EmergencyNameRevoked", index: 19, data: common },
  { pallet: "Names", event: "RegistrarSet", index: 20, data: { registrar: owner, enabled: true } },
];

test("finalized Host stream decodes every native Orbis Names event and its stable outcome", async () => {
  const source: TypedFinalizedEventSource = { async *subscribeFinalizedEvents() { yield { hash: finalized, events }; } };
  const received: FinalizedNamesOutcome[] = [];
  for await (const item of subscribeNamesEvents(source, namesEventSubscription(anchor, NAMES_EVENT_KINDS))) received.push(item);
  assert.deepEqual(received.map((item) => item.event.event.event), NAMES_EVENT_KINDS);
  assert.deepEqual(received.map((item) => item.outcome.outcome), NAMES_EVENT_KINDS);
  assert.deepEqual(received.map((item) => item.event.event_index), [...NAMES_EVENT_KINDS.keys()]);
});

test("descriptor-shaped Orbis Names bytes reject invalid UTF-8", () => {
  assert.throws(() => decodeNamesEvent({
    pallet: "Names", event: "TextSet", index: 0,
    data: { name, key: { value: Uint8Array.of(0xff) }, present: true },
  }), /valid UTF-8/);
});

test("finalized Host stream ignores non-Orbis Names and filters by requested kind", async () => {
  const source: TypedFinalizedEventSource = { async *subscribeFinalizedEvents() { yield { hash: finalized, events: [{ pallet: "Balances", event: "Transfer", index: 0, data: {} }, ...events] }; } };
  const received = [];
  for await (const item of subscribeNamesEvents(source, namesEventSubscription(anchor, ["name_registered"]))) received.push(item);
  assert.equal(received.length, 1);
  assert.equal(received[0]!.event.event.event, "name_registered");
});
