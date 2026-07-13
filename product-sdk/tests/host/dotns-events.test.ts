import assert from "node:assert/strict";
import test from "node:test";
import { decodeDotnsEvent, subscribeDotnsEvents, type FinalizedDotnsOutcome } from "../../packages/host/src/dotns-events.ts";
import type { TypedFinalizedEvent, TypedFinalizedEventSource } from "../../packages/host/src/attestation-events.ts";
import { DOTNS_EVENT_KINDS, dotnsEventSubscription } from "../../src/dotns.ts";
import type { BlockHash } from "../../src/types.ts";

const anchor = `0x${"11".repeat(32)}` as BlockHash;
const finalized = `0x${"22".repeat(32)}`;
const name = `0x${"33".repeat(32)}`;
const commitment = `0x${"44".repeat(32)}`;
const owner = "owner:alice";
const common = { name };
const events: readonly TypedFinalizedEvent[] = [
  { pallet: "Dotns", event: "CommitmentStored", index: 0, data: { owner, commitment, at: 1 } },
  { pallet: "Dotns", event: "CommitmentRemoved", index: 1, data: { owner, commitment } },
  { pallet: "Dotns", event: "NameRegistered", index: 2, data: { ...common, parent: null, label: { asBytes: () => new TextEncoder().encode("alice") }, owner, expires_at: 2 } },
  { pallet: "Dotns", event: "NameRenewed", index: 3, data: { ...common, expires_at: 3 } },
  { pallet: "Dotns", event: "NameTransferred", index: 4, data: { ...common, from: owner, to: "owner:bob" } },
  { pallet: "Dotns", event: "NameReleased", index: 5, data: { ...common, owner } },
  { pallet: "Dotns", event: "ExpiredNameRemoved", index: 6, data: common },
  { pallet: "Dotns", event: "ControllerAdded", index: 7, data: { ...common, controller: "controller:bob" } },
  { pallet: "Dotns", event: "ControllerRemoved", index: 8, data: { ...common, controller: "controller:bob" } },
  { pallet: "Dotns", event: "AddressSet", index: 9, data: { ...common, present: true } },
  { pallet: "Dotns", event: "SubjectSet", index: 10, data: { ...common, present: true } },
  { pallet: "Dotns", event: "AttestationSet", index: 11, data: { ...common, present: true } },
  { pallet: "Dotns", event: "ContentSet", index: 12, data: { ...common, present: true } },
  { pallet: "Dotns", event: "TextSet", index: 13, data: { ...common, key: new TextEncoder().encode("url"), present: true } },
  { pallet: "Dotns", event: "PrimaryNameSet", index: 14, data: { owner, name } },
  { pallet: "Dotns", event: "NameReserved", index: 15, data: { ...common, beneficiary: owner, expires_at: 4 } },
  { pallet: "Dotns", event: "ReservationCleared", index: 16, data: common },
  { pallet: "Dotns", event: "LabelProtectionSet", index: 17, data: { label: "0x726f6f74", protected: true } },
  { pallet: "Dotns", event: "PauseSet", index: 18, data: { paused: true } },
  { pallet: "Dotns", event: "EmergencyNameRevoked", index: 19, data: common },
  { pallet: "Dotns", event: "RegistrarSet", index: 20, data: { registrar: owner, enabled: true } },
];

test("finalized Host stream decodes every native DotNS event and its stable outcome", async () => {
  const source: TypedFinalizedEventSource = { async *subscribeFinalizedEvents() { yield { hash: finalized, events }; } };
  const received: FinalizedDotnsOutcome[] = [];
  for await (const item of subscribeDotnsEvents(source, dotnsEventSubscription(anchor, DOTNS_EVENT_KINDS))) received.push(item);
  assert.deepEqual(received.map((item) => item.event.event.event), DOTNS_EVENT_KINDS);
  assert.deepEqual(received.map((item) => item.outcome.outcome), DOTNS_EVENT_KINDS);
  assert.deepEqual(received.map((item) => item.event.event_index), [...DOTNS_EVENT_KINDS.keys()]);
});

test("descriptor-shaped DotNS bytes reject invalid UTF-8", () => {
  assert.throws(() => decodeDotnsEvent({
    pallet: "Dotns", event: "TextSet", index: 0,
    data: { name, key: { value: Uint8Array.of(0xff) }, present: true },
  }), /valid UTF-8/);
});

test("finalized Host stream ignores non-DotNS and filters by requested kind", async () => {
  const source: TypedFinalizedEventSource = { async *subscribeFinalizedEvents() { yield { hash: finalized, events: [{ pallet: "Balances", event: "Transfer", index: 0, data: {} }, ...events] }; } };
  const received = [];
  for await (const item of subscribeDotnsEvents(source, dotnsEventSubscription(anchor, ["name_registered"]))) received.push(item);
  assert.equal(received.length, 1);
  assert.equal(received[0]!.event.event.event, "name_registered");
});
