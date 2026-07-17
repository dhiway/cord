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

import type { CommonsChainClient } from "@cord-network/origin-sdk-chain-client";
import { COMMONS_NETWORK_BINDING } from "@cord-network/origin-sdk-descriptors";
import { OriginSdkError, type SdkResult } from "@cord-network/origin-sdk-errors";
import { blake2b256 } from "@cord-network/origin-sdk-crypto";
import { accountId, hash32, type AccountId, type Hash32, type Versioned } from "@cord-network/origin-sdk-identity";
import type { AttestationId } from "@cord-network/origin-sdk-attestation";
import { err } from "@cord-network/origin-sdk-result";
import { prepareAtFinalized, type PreparedTransaction } from "@cord-network/origin-sdk-tx";

export type { AccountId, AttestationId, Hash32, Versioned };

declare const nativeNamesType: unique symbol;
export type BlockHash = Hash32 & { readonly [nativeNamesType]: "BlockHash" };
export type NameId = Hash32 & { readonly [nativeNamesType]: "NameId" };
export type RegistrationCommitment = Hash32 & { readonly [nativeNamesType]: "RegistrationCommitment" };
export type RegistrationSalt = string & { readonly [nativeNamesType]: "RegistrationSalt" };
export type SubjectId = Hash32 & { readonly [nativeNamesType]: "SubjectId" };
export type ContentCommitment = Hash32 & { readonly [nativeNamesType]: "ContentCommitment" };
export type OperationId = string & { readonly [nativeNamesType]: "OperationId" };
export type BlockNumber = string & { readonly [nativeNamesType]: "BlockNumber" };

export interface PageInput { readonly cursor?: number | null; readonly limit?: number; }
export interface PageRequest { readonly cursor: number | null; readonly limit: number; }
export function page(input: PageInput = {}): PageRequest {
  const cursor = input.cursor ?? null;
  const limit = input.limit ?? 50;
  if (cursor !== null && (!Number.isSafeInteger(cursor) || cursor < 0 || cursor > 0xffff_ffff)) throw new TypeError("cursor must be a u32");
  if (!Number.isSafeInteger(limit) || limit < 0) throw new TypeError("limit must be non-negative");
  return { cursor, limit: Math.min(limit, 100) };
}
function invalidDomainInput(_domain: string, operation: string, message: string): never {
  throw new OriginSdkError({ source: "names", domain: operation, code: "invalid_input", message, retryable: false });
}
function nativeHash<Kind extends string>(value: string, label: string): string & { readonly __kind?: Kind } {
  if (!/^0x[0-9a-fA-F]{64}$/.test(value)) throw new TypeError(`${label} must be a 32-byte 0x-prefixed hash`);
  return value.toLowerCase() as string & { readonly __kind?: Kind };
}
export const blockHash = (value: string): BlockHash => nativeHash<"BlockHash">(value, "block hash") as BlockHash;
export const nameId = (value: string): NameId => nativeHash<"NameId">(value, "name id") as NameId;
export const registrationCommitment = (value: string): RegistrationCommitment => nativeHash<"RegistrationCommitment">(value, "registration commitment") as RegistrationCommitment;
export const subjectId = (value: string): SubjectId => nativeHash<"SubjectId">(value, "subject id") as SubjectId;
export const contentCommitment = (value: string): ContentCommitment => nativeHash<"ContentCommitment">(value, "content commitment") as ContentCommitment;
export function operationId(value: string): OperationId {
  if (!/^0x[0-9a-fA-F]{32}$/.test(value)) throw new TypeError("operation id must be a 16-byte 0x-prefixed value");
  return value.toLowerCase() as OperationId;
}
export function blockNumber(value: string | number): BlockNumber {
  const text = String(value);
  if (!/^(0|[1-9][0-9]*)$/.test(text) || BigInt(text) > 0xffff_ffffn) throw new TypeError("block number must be a u32");
  return text as BlockNumber;
}

declare const namesType: unique symbol;
export type NormalizedLabel = string & { readonly [namesType]: "NormalizedLabel" };
export type NamesAddress = string & { readonly [namesType]: "NamesAddress" };
export type TextKey = string & { readonly [namesType]: "TextKey" };
export type TextValue = string & { readonly [namesType]: "TextValue" };

const utf8 = new TextEncoder();
const NAME_ID_DOMAIN = "cord:orbis:names:name:v1";
const COMMITMENT_DOMAIN = "cord:orbis:names:commitment:v1";
const BASE58_ALPHABET = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
export const ATTESTATION_RESOLUTION_POLICY =
  "live-only: missing, revoked, expired, or inactive-schema attestations resolve to null" as const;

function concatBytes(...parts: readonly Uint8Array[]): Uint8Array {
  const output = new Uint8Array(parts.reduce((size, part) => size + part.length, 0));
  let offset = 0;
  for (const part of parts) { output.set(part, offset); offset += part.length; }
  return output;
}
function compactLength(value: number): Uint8Array {
  if (value < 64) return Uint8Array.of(value << 2);
  if (value < 16_384) { const encoded = (value << 2) | 1; return Uint8Array.of(encoded, encoded >>> 8); }
  invalidDomainInput("names", "canonical_encoding", "bounded byte length exceeds SCALE compact range");
}
function scaleBytes(value: Uint8Array): Uint8Array { return concatBytes(compactLength(value.length), value); }
function hashBytes(value: string, field: string): Uint8Array {
  if (!/^0x[0-9a-fA-F]{64}$/.test(value)) invalidDomainInput("names", "canonical_encoding", `${field} must be a 32-byte hash`);
  return Uint8Array.from(value.slice(2).match(/../g)!.map((pair) => Number.parseInt(pair, 16)));
}
function ss58AccountBytes(value: string): Uint8Array {
  let integer = 0n;
  for (const character of value) {
    const digit = BASE58_ALPHABET.indexOf(character);
    if (digit < 0) invalidDomainInput("names", "registration_commitment", "owner must be an SS58 account");
    integer = integer * 58n + BigInt(digit);
  }
  const decoded: number[] = [];
  while (integer > 0n) { decoded.push(Number(integer & 0xffn)); integer >>= 8n; }
  decoded.reverse();
  for (const character of value) { if (character !== "1") break; decoded.unshift(0); }
  const bytes = Uint8Array.from(decoded);
  if (bytes.length < 35 || bytes[0]! >= 128) invalidDomainInput("names", "registration_commitment", "owner must encode AccountId32");
  const prefixLength = (bytes[0]! & 0x40) === 0 ? 1 : 2;
  if (bytes.length !== prefixLength + 34) invalidDomainInput("names", "registration_commitment", "owner must encode AccountId32");
  return bytes.slice(prefixLength, prefixLength + 32);
}
function hashHex(value: Uint8Array): string { return `0x${Array.from(blake2b256(value), (byte) => byte.toString(16).padStart(2, "0")).join("")}`; }

/** Exact native pallet name identifier derivation. */
export function deriveNameId(genesisHash: BlockHash, parent: NameId | null, label: NormalizedLabel): NameId {
  const encoded = concatBytes(
    scaleBytes(utf8.encode(NAME_ID_DOMAIN)), hashBytes(genesisHash, "genesis_hash"),
    parent === null ? Uint8Array.of(0) : concatBytes(Uint8Array.of(1), hashBytes(parent, "parent")),
    scaleBytes(utf8.encode(normalizedLabel(label))),
  );
  return nativeHash<"NameId">(hashHex(encoded), "name_id") as NameId;
}

/** Exact native commit/reveal commitment derivation. */
export function deriveRegistrationCommitment(
  genesisHash: BlockHash, owner: AccountId, parent: NameId | null,
  label: NormalizedLabel, salt: RegistrationSalt,
): RegistrationCommitment {
  const name = deriveNameId(genesisHash, parent, label);
  const encoded = concatBytes(
    scaleBytes(utf8.encode(COMMITMENT_DOMAIN)), hashBytes(genesisHash, "genesis_hash"),
    ss58AccountBytes(owner), hashBytes(name, "name"), scaleBytes(utf8.encode(registrationSalt(salt))),
  );
  return nativeHash<"RegistrationCommitment">(hashHex(encoded), "registration_commitment") as RegistrationCommitment;
}

export function normalizedLabel(value: string): NormalizedLabel {
  if (value.length > 63 || !/^[a-z0-9](?:[a-z0-9-]*[a-z0-9])?$/.test(value)) {
    invalidDomainInput("names", "normalize_label", "label must follow lowercase ASCII label policy v1");
  }
  return value as NormalizedLabel;
}

export function registrationSalt(value: string): RegistrationSalt {
	const bytes = utf8.encode(value).length;
	if (bytes < 1 || bytes > 64) {
		invalidDomainInput("names", "registration_salt", "registration salt must contain 1-64 UTF-8 bytes");
	}
	return value as RegistrationSalt;
}

export function namesAddress(value: string): NamesAddress {
  if (!value || utf8.encode(value).length > 128) {
    invalidDomainInput("names", "address", "address must contain 1-128 UTF-8 bytes");
  }
  return value as NamesAddress;
}

export function textKey(value: string): TextKey {
  if (!value || utf8.encode(value).length > 32) {
    invalidDomainInput("names", "text_key", "text key must contain 1-32 UTF-8 bytes");
  }
  return value as TextKey;
}

export function textValue(value: string): TextValue {
	const bytes = utf8.encode(value).length;
	if (bytes < 1 || bytes > 256) {
		invalidDomainInput("names", "text_value", "text value must contain 1-256 UTF-8 bytes");
  }
  return value as TextValue;
}

export interface NameView {
  readonly name: NameId;
  readonly parent: NameId | null;
  readonly label: NormalizedLabel;
  readonly owner: AccountId;
  readonly expires_at: BlockNumber;
  readonly depth: number;
}

export interface NameStatus {
  readonly version: 1;
  readonly exists: boolean;
  readonly active: boolean;
  readonly expires_at: BlockNumber | null;
}
export interface ContentPublication { readonly content: ContentCommitment | null; readonly revision: bigint; }

export interface OwnerNamesPage {
  readonly version: 1;
  readonly names: readonly NameId[];
  readonly next_cursor: number | null;
}

export const NAMES_EVENT_KINDS = [
  "commitment_stored", "commitment_removed", "name_registered", "name_renewed",
  "name_transferred", "name_released", "expired_name_removed", "controller_added",
  "controller_removed", "address_set", "subject_set", "attestation_set", "content_set",
  "text_set", "primary_name_set", "name_reserved", "reservation_cleared",
  "label_protection_set", "pause_set", "emergency_name_revoked",
  "registrar_set",
] as const;
export type NamesEventKind = (typeof NAMES_EVENT_KINDS)[number];

export type NamesEvent =
  | { readonly event: "commitment_stored"; readonly data: { readonly owner: AccountId; readonly commitment: RegistrationCommitment; readonly at: BlockNumber } }
  | { readonly event: "commitment_removed"; readonly data: { readonly owner: AccountId; readonly commitment: RegistrationCommitment } }
  | { readonly event: "name_registered"; readonly data: { readonly name: NameId; readonly parent: NameId | null; readonly label: NormalizedLabel; readonly owner: AccountId; readonly expires_at: BlockNumber } }
  | { readonly event: "name_renewed"; readonly data: { readonly name: NameId; readonly expires_at: BlockNumber } }
  | { readonly event: "name_transferred"; readonly data: { readonly name: NameId; readonly from: AccountId; readonly to: AccountId } }
  | { readonly event: "name_released"; readonly data: { readonly name: NameId; readonly owner: AccountId } }
  | { readonly event: "expired_name_removed"; readonly data: { readonly name: NameId } }
  | { readonly event: "controller_added"; readonly data: { readonly name: NameId; readonly controller: AccountId } }
  | { readonly event: "controller_removed"; readonly data: { readonly name: NameId; readonly controller: AccountId } }
  | { readonly event: "address_set"; readonly data: { readonly name: NameId; readonly present: boolean } }
  | { readonly event: "subject_set"; readonly data: { readonly name: NameId; readonly present: boolean } }
  | { readonly event: "attestation_set"; readonly data: { readonly name: NameId; readonly present: boolean } }
  | { readonly event: "content_set"; readonly data: { readonly name: NameId; readonly present: boolean } }
  | { readonly event: "text_set"; readonly data: { readonly name: NameId; readonly key: TextKey; readonly present: boolean } }
  | { readonly event: "primary_name_set"; readonly data: { readonly owner: AccountId; readonly name: NameId | null } }
  | { readonly event: "name_reserved"; readonly data: { readonly name: NameId; readonly beneficiary: AccountId | null; readonly expires_at: BlockNumber | null } }
  | { readonly event: "reservation_cleared"; readonly data: { readonly name: NameId } }
  | { readonly event: "label_protection_set"; readonly data: { readonly label: NormalizedLabel; readonly protected: boolean } }
  | { readonly event: "pause_set"; readonly data: { readonly paused: boolean } }
  | { readonly event: "emergency_name_revoked"; readonly data: { readonly name: NameId } }
  | { readonly event: "registrar_set"; readonly data: { readonly registrar: AccountId; readonly enabled: boolean } };

export type NamesOutcome = { readonly outcome: NamesEventKind; readonly data: Readonly<Record<string, unknown>> };

export interface FinalizedNamesEvent {
  readonly finalized_block_hash: BlockHash;
  readonly event_index: number;
  readonly event: NamesEvent;
}

export interface NamesEventSubscription {
  readonly finality: "finalized";
  readonly from_finalized_block: BlockHash;
  readonly kinds: readonly NamesEventKind[];
}

export function namesEventOutcome(event: NamesEvent): NamesOutcome {
  switch (event.event) {
    case "commitment_stored": case "commitment_removed": return { outcome: event.event, data: { commitment: event.data.commitment } };
    case "name_transferred": return { outcome: event.event, data: { name: event.data.name, owner: event.data.to } };
    case "name_renewed": return { outcome: event.event, data: { name: event.data.name, expires_at: event.data.expires_at } };
    case "controller_added": case "controller_removed": return { outcome: event.event, data: { name: event.data.name, controller: event.data.controller } };
    case "address_set": case "subject_set": case "attestation_set": case "content_set": return { outcome: event.event, data: { name: event.data.name, present: event.data.present } };
    case "text_set": return { outcome: event.event, data: { name: event.data.name, key: event.data.key, present: event.data.present } };
    case "primary_name_set": return { outcome: event.event, data: { owner: event.data.owner, name: event.data.name } };
    case "label_protection_set": return { outcome: event.event, data: { label: event.data.label, protected: event.data.protected } };
    case "pause_set": return { outcome: event.event, data: { paused: event.data.paused } };
    case "registrar_set": return { outcome: event.event, data: { registrar: event.data.registrar, enabled: event.data.enabled } };
    default: return { outcome: event.event, data: { name: event.data.name } };
  }
}

export function namesEventSubscription(
  from_finalized_block: BlockHash,
  kinds: readonly NamesEventKind[],
): NamesEventSubscription {
  nativeHash(from_finalized_block, "from_finalized_block");
  if (kinds.length < 1 || kinds.length > 21 || new Set(kinds).size !== kinds.length ||
      kinds.some((kind) => !NAMES_EVENT_KINDS.includes(kind))) {
    invalidDomainInput("names", "subscribe_events", "subscription requires 1-21 unique Orbis Names event kinds");
  }
  return { finality: "finalized", from_finalized_block, kinds };
}


export interface NamesRuntimeAdapter {
  labelPolicyVersion(at: `0x${string}`, signal?: AbortSignal): Promise<number>;
  nameById(at: `0x${string}`, name: NameId, signal?: AbortSignal): Promise<Versioned<NameView>>;
  rootNameByNormalizedLabel(at: `0x${string}`, label: NormalizedLabel, signal?: AbortSignal): Promise<Versioned<NameId>>;
  ownerNames(at: `0x${string}`, owner: AccountId, request: PageRequest, signal?: AbortSignal): Promise<OwnerNamesPage>;
  controllers(at: `0x${string}`, name: NameId, signal?: AbortSignal): Promise<Versioned<readonly AccountId[]>>;
  resolveAddress(at: `0x${string}`, name: NameId, signal?: AbortSignal): Promise<Versioned<NamesAddress>>;
  resolveSubject(at: `0x${string}`, name: NameId, signal?: AbortSignal): Promise<Versioned<SubjectId>>;
  resolveAttestation(at: `0x${string}`, name: NameId, signal?: AbortSignal): Promise<Versioned<AttestationId>>;
  resolveContentPublication(at: `0x${string}`, name: NameId, signal?: AbortSignal): Promise<Versioned<ContentPublication>>;
  resolveText(at: `0x${string}`, name: NameId, key: TextKey, signal?: AbortSignal): Promise<Versioned<TextValue>>;
  primaryName(at: `0x${string}`, owner: AccountId, signal?: AbortSignal): Promise<Versioned<NameId>>;
  nameStatus(at: `0x${string}`, name: NameId, signal?: AbortSignal): Promise<NameStatus>;
  commit(at: `0x${string}`, commitment: RegistrationCommitment, signal?: AbortSignal): Promise<PreparedTransaction>;
  cancelCommitment(at: `0x${string}`, commitment: RegistrationCommitment, signal?: AbortSignal): Promise<PreparedTransaction>;
  pruneExpiredCommitment(at: `0x${string}`, owner: AccountId, commitment: RegistrationCommitment, signal?: AbortSignal): Promise<PreparedTransaction>;
  register(at: `0x${string}`, parent: NameId | null, label: NormalizedLabel, salt: RegistrationSalt, signal?: AbortSignal): Promise<PreparedTransaction>;
  renew(at: `0x${string}`, name: NameId, additionalPeriod: BlockNumber, signal?: AbortSignal): Promise<PreparedTransaction>;
  transfer(at: `0x${string}`, name: NameId, newOwner: AccountId, signal?: AbortSignal): Promise<PreparedTransaction>;
  addController(at: `0x${string}`, name: NameId, controller: AccountId, signal?: AbortSignal): Promise<PreparedTransaction>;
  removeController(at: `0x${string}`, name: NameId, controller: AccountId, signal?: AbortSignal): Promise<PreparedTransaction>;
  setAddress(at: `0x${string}`, name: NameId, address: NamesAddress | null, signal?: AbortSignal): Promise<PreparedTransaction>;
  setSubject(at: `0x${string}`, name: NameId, subject: SubjectId | null, signal?: AbortSignal): Promise<PreparedTransaction>;
  setAttestation(at: `0x${string}`, name: NameId, attestation: AttestationId | null, signal?: AbortSignal): Promise<PreparedTransaction>;
  publishContent(at: `0x${string}`, name: NameId, content: ContentCommitment | null, expectedRevision: string, operationId: OperationId, signal?: AbortSignal): Promise<PreparedTransaction>;
  setText(at: `0x${string}`, name: NameId, key: TextKey, value: TextValue | null, signal?: AbortSignal): Promise<PreparedTransaction>;
  setPrimaryName(at: `0x${string}`, name: NameId | null, signal?: AbortSignal): Promise<PreparedTransaction>;
  release(at: `0x${string}`, name: NameId, signal?: AbortSignal): Promise<PreparedTransaction>;
  removeExpiredName(at: `0x${string}`, name: NameId, signal?: AbortSignal): Promise<PreparedTransaction>;
}

export interface NamesClient {
  labelPolicyVersion(signal?: AbortSignal): Promise<SdkResult<number>>;
  nameById(name: NameId, signal?: AbortSignal): Promise<SdkResult<Versioned<NameView>>>;
  rootNameByNormalizedLabel(label: NormalizedLabel, signal?: AbortSignal): Promise<SdkResult<Versioned<NameId>>>;
  ownerNames(owner: AccountId, input?: PageInput, signal?: AbortSignal): Promise<SdkResult<OwnerNamesPage>>;
  controllers(name: NameId, signal?: AbortSignal): Promise<SdkResult<Versioned<readonly AccountId[]>>>;
  resolveAddress(name: NameId, signal?: AbortSignal): Promise<SdkResult<Versioned<NamesAddress>>>;
  resolveSubject(name: NameId, signal?: AbortSignal): Promise<SdkResult<Versioned<SubjectId>>>;
  resolveAttestation(name: NameId, signal?: AbortSignal): Promise<SdkResult<Versioned<AttestationId>>>;
  resolveContentPublication(name: NameId, signal?: AbortSignal): Promise<SdkResult<Versioned<ContentPublication>>>;
  resolveText(name: NameId, key: TextKey, signal?: AbortSignal): Promise<SdkResult<Versioned<TextValue>>>;
  primaryName(owner: AccountId, signal?: AbortSignal): Promise<SdkResult<Versioned<NameId>>>;
  nameStatus(name: NameId, signal?: AbortSignal): Promise<SdkResult<NameStatus>>;
  prepareCommit(commitment: RegistrationCommitment, signal?: AbortSignal): Promise<SdkResult<PreparedTransaction>>;
  prepareCancelCommitment(commitment: RegistrationCommitment, signal?: AbortSignal): Promise<SdkResult<PreparedTransaction>>;
  preparePruneExpiredCommitment(owner: AccountId, commitment: RegistrationCommitment, signal?: AbortSignal): Promise<SdkResult<PreparedTransaction>>;
  prepareRegister(parent: NameId | null, label: NormalizedLabel, salt: RegistrationSalt, signal?: AbortSignal): Promise<SdkResult<PreparedTransaction>>;
  prepareRenew(name: NameId, additionalPeriod: BlockNumber, signal?: AbortSignal): Promise<SdkResult<PreparedTransaction>>;
  prepareTransfer(name: NameId, newOwner: AccountId, signal?: AbortSignal): Promise<SdkResult<PreparedTransaction>>;
  prepareAddController(name: NameId, controller: AccountId, signal?: AbortSignal): Promise<SdkResult<PreparedTransaction>>;
  prepareRemoveController(name: NameId, controller: AccountId, signal?: AbortSignal): Promise<SdkResult<PreparedTransaction>>;
  prepareSetAddress(name: NameId, address: NamesAddress | null, signal?: AbortSignal): Promise<SdkResult<PreparedTransaction>>;
  prepareSetSubject(name: NameId, subject: SubjectId | null, signal?: AbortSignal): Promise<SdkResult<PreparedTransaction>>;
  prepareSetAttestation(name: NameId, attestation: AttestationId | null, signal?: AbortSignal): Promise<SdkResult<PreparedTransaction>>;
  preparePublishContent(name: NameId, content: ContentCommitment | null, expectedRevision: string, operationId: OperationId, signal?: AbortSignal): Promise<SdkResult<PreparedTransaction>>;
  prepareSetText(name: NameId, key: TextKey, value: TextValue | null, signal?: AbortSignal): Promise<SdkResult<PreparedTransaction>>;
  prepareSetPrimaryName(name: NameId | null, signal?: AbortSignal): Promise<SdkResult<PreparedTransaction>>;
  prepareRelease(name: NameId, signal?: AbortSignal): Promise<SdkResult<PreparedTransaction>>;
  prepareRemoveExpiredName(name: NameId, signal?: AbortSignal): Promise<SdkResult<PreparedTransaction>>;
}

export const NAMES_ADMIN_EXCLUSIONS = [
  { target: "Names.reserve_name", reason: "registrar administration" },
  { target: "Names.clear_reservation", reason: "registrar administration" },
  { target: "Names.set_label_protection", reason: "namespace policy administration" },
  { target: "Names.set_paused", reason: "runtime safety administration" },
  { target: "Names.force_transfer", reason: "forced runtime administration" },
  { target: "Names.force_revoke", reason: "forced runtime administration" },
  { target: "Names.set_registrar", reason: "registrar administration" },
] as const;

export const NAMES_NATIVE_BINDINGS = {
  metadataHash: COMMONS_NETWORK_BINDING.metadata_hash,
  runtimeApi: "NamesApi.v1",
  pallet: "Names",
  reads: ["label_policy_version", "name_by_id", "root_name_by_normalized_label", "owner_names", "controllers", "resolve_address", "resolve_subject", "resolve_attestation", "resolve_content_publication", "resolve_text", "primary_name", "name_status"],
  transactions: ["commit", "cancel_commitment", "prune_expired_commitment", "register", "renew", "transfer", "add_controller", "remove_controller", "set_address", "set_subject", "set_attestation", "publish_content", "set_text", "set_primary_name", "release", "remove_expired_name"],
} as const;

const invalidResult = <T>(message: string): SdkResult<T> => err(new OriginSdkError({
  source: "names", domain: "input", code: "invalid_input", message, retryable: false,
}));

export function createNamesClient(chain: CommonsChainClient, runtime: NamesRuntimeAdapter): NamesClient {
  const prepare = (builder: (at: `0x${string}`) => Promise<PreparedTransaction>, signal?: AbortSignal) =>
    prepareAtFinalized(chain, ({ block }) => builder(block.hash), signal);
  const checked = async <T>(validate: () => void, operation: () => Promise<SdkResult<T>>): Promise<SdkResult<T>> => {
    try { validate(); } catch (error) { return invalidResult(error instanceof Error ? error.message : "invalid names input"); }
    return operation();
  };
  const validName = (value: NameId) => { nameId(value); };
  const validOptionalName = (value: NameId | null) => { if (value !== null) nameId(value); };
  return {
    labelPolicyVersion: (signal) => chain.readFinalized((at) => runtime.labelPolicyVersion(at, signal), signal),
    nameById: (name, signal) => checked(() => validName(name), () => chain.readFinalized((at) => runtime.nameById(at, name, signal), signal)),
    rootNameByNormalizedLabel: (label, signal) => checked(() => { normalizedLabel(label); }, () => chain.readFinalized((at) => runtime.rootNameByNormalizedLabel(at, label, signal), signal)),
    ownerNames: (owner, input, signal) => checked(() => { accountId(owner); page(input); }, () => chain.readFinalized((at) => runtime.ownerNames(at, owner, page(input), signal), signal)),
    controllers: (name, signal) => checked(() => validName(name), () => chain.readFinalized((at) => runtime.controllers(at, name, signal), signal)),
    resolveAddress: (name, signal) => checked(() => validName(name), () => chain.readFinalized((at) => runtime.resolveAddress(at, name, signal), signal)),
    resolveSubject: (name, signal) => checked(() => validName(name), () => chain.readFinalized((at) => runtime.resolveSubject(at, name, signal), signal)),
    resolveAttestation: (name, signal) => checked(() => validName(name), () => chain.readFinalized((at) => runtime.resolveAttestation(at, name, signal), signal)),
    resolveContentPublication: (name, signal) => checked(() => validName(name), () => chain.readFinalized((at) => runtime.resolveContentPublication(at, name, signal), signal)),
    resolveText: (name, key, signal) => checked(() => { validName(name); textKey(key); }, () => chain.readFinalized((at) => runtime.resolveText(at, name, key, signal), signal)),
    primaryName: (owner, signal) => checked(() => { accountId(owner); }, () => chain.readFinalized((at) => runtime.primaryName(at, owner, signal), signal)),
    nameStatus: (name, signal) => checked(() => validName(name), () => chain.readFinalized((at) => runtime.nameStatus(at, name, signal), signal)),
    prepareCommit: (commitment, signal) => checked(() => { registrationCommitment(commitment); }, () => prepare((at) => runtime.commit(at, commitment, signal), signal)),
    prepareCancelCommitment: (commitment, signal) => checked(() => { registrationCommitment(commitment); }, () => prepare((at) => runtime.cancelCommitment(at, commitment, signal), signal)),
    preparePruneExpiredCommitment: (owner, commitment, signal) => checked(() => { accountId(owner); registrationCommitment(commitment); }, () => prepare((at) => runtime.pruneExpiredCommitment(at, owner, commitment, signal), signal)),
    prepareRegister: (parent, label, salt, signal) => checked(() => { validOptionalName(parent); normalizedLabel(label); registrationSalt(salt); }, () => prepare((at) => runtime.register(at, parent, label, salt, signal), signal)),
    prepareRenew: (name, period, signal) => checked(() => { validName(name); blockNumber(period); }, () => prepare((at) => runtime.renew(at, name, period, signal), signal)),
    prepareTransfer: (name, owner, signal) => checked(() => { validName(name); accountId(owner); }, () => prepare((at) => runtime.transfer(at, name, owner, signal), signal)),
    prepareAddController: (name, controller, signal) => checked(() => { validName(name); accountId(controller); }, () => prepare((at) => runtime.addController(at, name, controller, signal), signal)),
    prepareRemoveController: (name, controller, signal) => checked(() => { validName(name); accountId(controller); }, () => prepare((at) => runtime.removeController(at, name, controller, signal), signal)),
    prepareSetAddress: (name, address, signal) => checked(() => { validName(name); if (address !== null) namesAddress(address); }, () => prepare((at) => runtime.setAddress(at, name, address, signal), signal)),
    prepareSetSubject: (name, subject, signal) => checked(() => { validName(name); if (subject !== null) subjectId(subject); }, () => prepare((at) => runtime.setSubject(at, name, subject, signal), signal)),
    prepareSetAttestation: (name, attestation, signal) => checked(() => { validName(name); if (attestation !== null) hash32(attestation); }, () => prepare((at) => runtime.setAttestation(at, name, attestation, signal), signal)),
    preparePublishContent: (name, content, expectedRevision, id, signal) => checked(() => {
      validName(name); if (content !== null) contentCommitment(content); operationId(id);
      if (!/^(0|[1-9][0-9]*)$/.test(expectedRevision) || BigInt(expectedRevision) > 0xffff_ffff_ffff_ffffn) throw new TypeError("expected revision must be a u64 decimal string");
    }, () => prepare((at) => runtime.publishContent(at, name, content, expectedRevision, id, signal), signal)),
    prepareSetText: (name, key, value, signal) => checked(() => { validName(name); textKey(key); if (value !== null) textValue(value); }, () => prepare((at) => runtime.setText(at, name, key, value, signal), signal)),
    prepareSetPrimaryName: (name, signal) => checked(() => validOptionalName(name), () => prepare((at) => runtime.setPrimaryName(at, name, signal), signal)),
    prepareRelease: (name, signal) => checked(() => validName(name), () => prepare((at) => runtime.release(at, name, signal), signal)),
    prepareRemoveExpiredName: (name, signal) => checked(() => validName(name), () => prepare((at) => runtime.removeExpiredName(at, name, signal), signal)),
  };
}

export type NameByIdResponse = Versioned<NameView>;
export type ResolvedAddress = Versioned<NamesAddress>;
export type ResolvedSubject = Versioned<SubjectId>;
export type ResolvedAttestation = Versioned<AttestationId>;
export type ResolvedContent = Versioned<ContentCommitment>;
export type ResolvedText = Versioned<TextValue>;
