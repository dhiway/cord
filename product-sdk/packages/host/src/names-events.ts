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

import { ProductSdkError } from "../../core/src/contract.ts";
import type { TypedFinalizedEvent, TypedFinalizedEventSource } from "./attestation-events.ts";
import {
  namesEventOutcome,
  normalizedLabel,
  textKey,
  type NamesEvent,
  type NamesEventKind,
  type NamesEventSubscription,
  type NamesOutcome,
  type FinalizedNamesEvent,
} from "../../../src/names.ts";
import type {
  AccountId, BlockHash, BlockNumber, NameId, RegistrationCommitment,
} from "../../../src/types.ts";

export interface FinalizedNamesOutcome { readonly event: FinalizedNamesEvent; readonly outcome: NamesOutcome }
const HASH = /^0x[0-9a-f]{64}$/i;
const DECIMAL = /^(0|[1-9][0-9]*)$/;
function rejected(message: string): never { throw new ProductSdkError("runtime_rejected", message); }
function hash(value: unknown, field: string): string {
  if (typeof value !== "string" || !HASH.test(value)) rejected(`Names.${field} is not a hash`);
  return value.toLowerCase();
}
function account(value: unknown, field: string): AccountId {
  if (typeof value !== "string" || value.length < 1 || value.length > 128) rejected(`Names.${field} is not an account`);
  return value as AccountId;
}
function bool(value: unknown, field: string): boolean {
  if (typeof value !== "boolean") rejected(`Names.${field} is not boolean`);
  return value;
}
function block(value: unknown, field: string): BlockNumber {
  const text = typeof value === "bigint" || typeof value === "number" || typeof value === "string" ? String(value) : "";
  if (!DECIMAL.test(text) || BigInt(text) > 0xffff_ffffn) rejected(`Names.${field} is not a u32 block number`);
  return text as BlockNumber;
}
function nullableHash(value: unknown, field: string): NameId | null {
  return value === null || value === undefined ? null : hash(value, field) as NameId;
}
function nullableAccount(value: unknown, field: string): AccountId | null {
  return value === null || value === undefined ? null : account(value, field);
}
function utf8Bytes(value: unknown, field: string): string {
  if (typeof value === "string" && !value.startsWith("0x")) return value;
  let bytes: Uint8Array | undefined;
  if (typeof value === "string" && /^0x(?:[0-9a-fA-F]{2})*$/.test(value)) {
    bytes = Uint8Array.from(value.slice(2).match(/../g)?.map((pair) => Number.parseInt(pair, 16)) ?? []);
  } else if (value instanceof Uint8Array) {
    bytes = value;
  } else if (Array.isArray(value) && value.every((item) => Number.isInteger(item) && item >= 0 && item <= 255)) {
    bytes = Uint8Array.from(value);
  } else if (value && typeof value === "object") {
    const binary = value as { asBytes?: () => unknown; toHex?: () => unknown; value?: unknown };
    if (typeof binary.asBytes === "function") return utf8Bytes(binary.asBytes(), field);
    if (typeof binary.toHex === "function") return utf8Bytes(binary.toHex(), field);
    if ("value" in binary) return utf8Bytes(binary.value, field);
  }
  if (!bytes) rejected(`Names.${field} is not descriptor-shaped bytes`);
  try {
    return new TextDecoder("utf-8", { fatal: true }).decode(bytes);
  } catch {
    rejected(`Names.${field} is not valid UTF-8`);
  }
}
function dataObject(native: TypedFinalizedEvent): Readonly<Record<string, unknown>> {
  if (!native.data || typeof native.data !== "object" || Array.isArray(native.data)) rejected(`Names.${native.event} data is not an object`);
  return native.data;
}

/** Decode one exact native `Names` pallet event into the stable product DTO. */
export function decodeNamesEvent(native: TypedFinalizedEvent): NamesEvent | null {
  if (native.pallet !== "Names") return null;
  const d = dataObject(native);
  const name = () => hash(d.name, "name") as NameId;
  const present = () => bool(d.present, "present");
  switch (native.event) {
    case "CommitmentStored": return { event: "commitment_stored", data: { owner: account(d.owner, "owner"), commitment: hash(d.commitment, "commitment") as RegistrationCommitment, at: block(d.at, "at") } };
    case "CommitmentRemoved": return { event: "commitment_removed", data: { owner: account(d.owner, "owner"), commitment: hash(d.commitment, "commitment") as RegistrationCommitment } };
    case "NameRegistered": return { event: "name_registered", data: { name: name(), parent: nullableHash(d.parent, "parent"), label: normalizedLabel(utf8Bytes(d.label, "label")), owner: account(d.owner, "owner"), expires_at: block(d.expires_at, "expires_at") } };
    case "NameRenewed": return { event: "name_renewed", data: { name: name(), expires_at: block(d.expires_at, "expires_at") } };
    case "NameTransferred": return { event: "name_transferred", data: { name: name(), from: account(d.from, "from"), to: account(d.to, "to") } };
    case "NameReleased": return { event: "name_released", data: { name: name(), owner: account(d.owner, "owner") } };
    case "ExpiredNameRemoved": return { event: "expired_name_removed", data: { name: name() } };
    case "ControllerAdded": return { event: "controller_added", data: { name: name(), controller: account(d.controller, "controller") } };
    case "ControllerRemoved": return { event: "controller_removed", data: { name: name(), controller: account(d.controller, "controller") } };
    case "AddressSet": return { event: "address_set", data: { name: name(), present: present() } };
    case "SubjectSet": return { event: "subject_set", data: { name: name(), present: present() } };
    case "AttestationSet": return { event: "attestation_set", data: { name: name(), present: present() } };
    case "ContentSet": return { event: "content_set", data: { name: name(), present: present() } };
    case "TextSet": return { event: "text_set", data: { name: name(), key: textKey(utf8Bytes(d.key, "key")), present: present() } };
    case "PrimaryNameSet": return { event: "primary_name_set", data: { owner: account(d.owner, "owner"), name: nullableHash(d.name, "name") } };
    case "NameReserved": return { event: "name_reserved", data: { name: name(), beneficiary: nullableAccount(d.beneficiary, "beneficiary"), expires_at: d.expires_at === null || d.expires_at === undefined ? null : block(d.expires_at, "expires_at") } };
    case "ReservationCleared": return { event: "reservation_cleared", data: { name: name() } };
    case "LabelProtectionSet": return { event: "label_protection_set", data: { label: normalizedLabel(utf8Bytes(d.label, "label")), protected: bool(d.protected, "protected") } };
    case "PauseSet": return { event: "pause_set", data: { paused: bool(d.paused, "paused") } };
    case "EmergencyNameRevoked": return { event: "emergency_name_revoked", data: { name: name() } };
    case "RegistrarSet": return { event: "registrar_set", data: { registrar: account(d.registrar, "registrar"), enabled: bool(d.enabled, "enabled") } };
    default: throw new ProductSdkError("unsupported_runtime", `unknown native Names event ${native.event}`);
  }
}

export async function* subscribeNamesEvents(
  source: TypedFinalizedEventSource,
  subscription: NamesEventSubscription,
  signal: AbortSignal = new AbortController().signal,
): AsyncGenerator<FinalizedNamesOutcome> {
  const wanted = new Set<NamesEventKind>(subscription.kinds);
  for await (const blockResult of source.subscribeFinalizedEvents({ from: subscription.from_finalized_block, signal })) {
    if (signal.aborted) throw new ProductSdkError("cancelled", "Orbis Names subscription cancelled");
    const finalizedBlockHash = hash(blockResult.hash, "finalized_block_hash") as BlockHash;
    for (const native of blockResult.events) {
      if (!Number.isSafeInteger(native.index) || native.index < 0 || native.index > 0xffff_ffff) rejected("Orbis Names event index is not a u32");
      const event = decodeNamesEvent(native);
      if (!event || !wanted.has(event.event)) continue;
      const finalized: FinalizedNamesEvent = { finalized_block_hash: finalizedBlockHash, event_index: native.index, event };
      yield { event: finalized, outcome: namesEventOutcome(event) };
    }
  }
}
