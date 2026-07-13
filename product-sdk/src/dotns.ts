import { finalizedRead, submitAndFinalize, type RequestContext } from "./host.ts";
import {
  page,
  type AccountId,
  type AttestationId,
  type BlockHash,
  type BlockNumber,
  type ContentCommitment,
  type NameId,
  type PageInput,
  type RegistrationCommitment,
  type RegistrationSalt,
  type SubjectId,
  type Versioned,
  nativeHash,
} from "./types.ts";
import { invalidDomainInput } from "./errors.ts";
import { digestContent } from "./content.ts";

declare const dotnsType: unique symbol;
export type NormalizedLabel = string & { readonly [dotnsType]: "NormalizedLabel" };
export type DotnsAddress = string & { readonly [dotnsType]: "DotnsAddress" };
export type TextKey = string & { readonly [dotnsType]: "TextKey" };
export type TextValue = string & { readonly [dotnsType]: "TextValue" };

const utf8 = new TextEncoder();
const NAME_ID_DOMAIN = "cord:orbis:dotns:name:v1";
const COMMITMENT_DOMAIN = "cord:orbis:dotns:commitment:v1";
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
  invalidDomainInput("dotns", "canonical_encoding", "bounded byte length exceeds SCALE compact range");
}
function scaleBytes(value: Uint8Array): Uint8Array { return concatBytes(compactLength(value.length), value); }
function hashBytes(value: string, field: string): Uint8Array {
  if (!/^0x[0-9a-fA-F]{64}$/.test(value)) invalidDomainInput("dotns", "canonical_encoding", `${field} must be a 32-byte hash`);
  return Uint8Array.from(value.slice(2).match(/../g)!.map((pair) => Number.parseInt(pair, 16)));
}
function ss58AccountBytes(value: string): Uint8Array {
  let integer = 0n;
  for (const character of value) {
    const digit = BASE58_ALPHABET.indexOf(character);
    if (digit < 0) invalidDomainInput("dotns", "registration_commitment", "owner must be an SS58 account");
    integer = integer * 58n + BigInt(digit);
  }
  const decoded: number[] = [];
  while (integer > 0n) { decoded.push(Number(integer & 0xffn)); integer >>= 8n; }
  decoded.reverse();
  for (const character of value) { if (character !== "1") break; decoded.unshift(0); }
  const bytes = Uint8Array.from(decoded);
  if (bytes.length < 35 || bytes[0]! >= 128) invalidDomainInput("dotns", "registration_commitment", "owner must encode AccountId32");
  const prefixLength = (bytes[0]! & 0x40) === 0 ? 1 : 2;
  if (bytes.length !== prefixLength + 34) invalidDomainInput("dotns", "registration_commitment", "owner must encode AccountId32");
  return bytes.slice(prefixLength, prefixLength + 32);
}
function hashHex(value: Uint8Array): string { return `0x${Array.from(digestContent("blake2b-256", value), (byte) => byte.toString(16).padStart(2, "0")).join("")}`; }

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
    invalidDomainInput("dotns", "normalize_label", "label must follow lowercase ASCII label policy v1");
  }
  return value as NormalizedLabel;
}

export function registrationSalt(value: string): RegistrationSalt {
	const bytes = utf8.encode(value).length;
	if (bytes < 1 || bytes > 64) {
		invalidDomainInput("dotns", "registration_salt", "registration salt must contain 1-64 UTF-8 bytes");
	}
	return value as RegistrationSalt;
}

export function dotnsAddress(value: string): DotnsAddress {
  if (!value || utf8.encode(value).length > 128) {
    invalidDomainInput("dotns", "address", "address must contain 1-128 UTF-8 bytes");
  }
  return value as DotnsAddress;
}

export function textKey(value: string): TextKey {
  if (!value || utf8.encode(value).length > 32) {
    invalidDomainInput("dotns", "text_key", "text key must contain 1-32 UTF-8 bytes");
  }
  return value as TextKey;
}

export function textValue(value: string): TextValue {
	const bytes = utf8.encode(value).length;
	if (bytes < 1 || bytes > 256) {
		invalidDomainInput("dotns", "text_value", "text value must contain 1-256 UTF-8 bytes");
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

export interface OwnerNamesPage {
  readonly version: 1;
  readonly names: readonly NameId[];
  readonly next_cursor: number | null;
}

export const DOTNS_EVENT_KINDS = [
  "commitment_stored", "commitment_removed", "name_registered", "name_renewed",
  "name_transferred", "name_released", "expired_name_removed", "controller_added",
  "controller_removed", "address_set", "subject_set", "attestation_set", "content_set",
  "text_set", "primary_name_set", "name_reserved", "reservation_cleared",
  "label_protection_set", "pause_set", "emergency_name_revoked",
  "registrar_set",
] as const;
export type DotnsEventKind = (typeof DOTNS_EVENT_KINDS)[number];

export type DotnsEvent =
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

export type DotnsOutcome = { readonly outcome: DotnsEventKind; readonly data: Readonly<Record<string, unknown>> };

export interface FinalizedDotnsEvent {
  readonly finalized_block_hash: BlockHash;
  readonly event_index: number;
  readonly event: DotnsEvent;
}

export interface DotnsEventSubscription {
  readonly finality: "finalized";
  readonly from_finalized_block: BlockHash;
  readonly kinds: readonly DotnsEventKind[];
}

export function dotnsEventOutcome(event: DotnsEvent): DotnsOutcome {
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

export function dotnsEventSubscription(
  from_finalized_block: BlockHash,
  kinds: readonly DotnsEventKind[],
): DotnsEventSubscription {
  nativeHash(from_finalized_block, "from_finalized_block");
  if (kinds.length < 1 || kinds.length > 21 || new Set(kinds).size !== kinds.length ||
      kinds.some((kind) => !DOTNS_EVENT_KINDS.includes(kind))) {
    invalidDomainInput("dotns", "subscribe_events", "subscription requires 1-21 unique DotNS event kinds");
  }
  return { finality: "finalized", from_finalized_block, kinds };
}

export const dotns = {
	labelPolicyVersion(context: RequestContext) {
		return finalizedRead("dotns", context, "dotns", "label_policy_version", {});
	},

  nameById(context: RequestContext, name: NameId) {
    return finalizedRead("dotns", context, "dotns", "name_by_id", { name });
  },

  rootNameByNormalizedLabel(context: RequestContext, label: NormalizedLabel) {
    return finalizedRead("dotns", context, "dotns", "root_name_by_normalized_label", { label });
  },

  ownerNames(context: RequestContext, owner: AccountId, input?: PageInput) {
    return finalizedRead("dotns", context, "dotns", "owner_names", { owner, ...page(input) });
  },

  controllers(context: RequestContext, name: NameId) {
    return finalizedRead("dotns", context, "dotns", "controllers", { name });
  },

  resolveAddress(context: RequestContext, name: NameId) {
    return finalizedRead("dotns", context, "dotns", "resolve_address", { name });
  },

  resolveSubject(context: RequestContext, name: NameId) {
    return finalizedRead("dotns", context, "dotns", "resolve_subject", { name });
  },

  resolveAttestation(context: RequestContext, name: NameId) {
    return finalizedRead("dotns", context, "dotns", "resolve_attestation", { name });
  },

  resolveContent(context: RequestContext, name: NameId) {
    return finalizedRead("dotns", context, "dotns", "resolve_content", { name });
  },

  resolveText(context: RequestContext, name: NameId, key: TextKey) {
    return finalizedRead("dotns", context, "dotns", "resolve_text", { name, key });
  },

  primaryName(context: RequestContext, owner: AccountId) {
    return finalizedRead("dotns", context, "dotns", "primary_name", { owner });
  },

  nameStatus(context: RequestContext, name: NameId) {
    return finalizedRead("dotns", context, "dotns", "name_status", { name });
  },

  commit(context: RequestContext, commitment: RegistrationCommitment) {
    return submitAndFinalize("dotns", context, "dotns", "commit", { commitment });
  },

  cancelCommitment(context: RequestContext, commitment: RegistrationCommitment) {
    return submitAndFinalize("dotns", context, "dotns", "cancel_commitment", { commitment });
  },

  pruneExpiredCommitment(
    context: RequestContext,
    owner: AccountId,
    commitment: RegistrationCommitment,
  ) {
    return submitAndFinalize("dotns", context, "dotns", "prune_expired_commitment", {
      owner,
      commitment,
    });
  },

  register(
    context: RequestContext,
    parent: NameId | null,
    label: NormalizedLabel,
    salt: RegistrationSalt,
  ) {
    return submitAndFinalize("dotns", context, "dotns", "register", { parent, label, salt });
  },

  renew(context: RequestContext, name: NameId, additional_period: BlockNumber) {
    return submitAndFinalize("dotns", context, "dotns", "renew", { name, additional_period });
  },

  transfer(context: RequestContext, name: NameId, new_owner: AccountId) {
    return submitAndFinalize("dotns", context, "dotns", "transfer", { name, new_owner });
  },

  addController(context: RequestContext, name: NameId, controller: AccountId) {
    return submitAndFinalize("dotns", context, "dotns", "add_controller", { name, controller });
  },

  removeController(context: RequestContext, name: NameId, controller: AccountId) {
    return submitAndFinalize("dotns", context, "dotns", "remove_controller", { name, controller });
  },

  setAddress(context: RequestContext, name: NameId, address: DotnsAddress | null) {
    return submitAndFinalize("dotns", context, "dotns", "set_address", { name, address });
  },

  setSubject(context: RequestContext, name: NameId, subject: SubjectId | null) {
    return submitAndFinalize("dotns", context, "dotns", "set_subject", { name, subject });
  },

  setAttestation(context: RequestContext, name: NameId, attestation: AttestationId | null) {
    return submitAndFinalize("dotns", context, "dotns", "set_attestation", { name, attestation });
  },

  setContent(context: RequestContext, name: NameId, content: ContentCommitment | null) {
    return submitAndFinalize("dotns", context, "dotns", "set_content", { name, content });
  },

  setText(context: RequestContext, name: NameId, key: TextKey, value: TextValue | null) {
    return submitAndFinalize("dotns", context, "dotns", "set_text", { name, key, value });
  },

  setPrimaryName(context: RequestContext, name: NameId | null) {
    return submitAndFinalize("dotns", context, "dotns", "set_primary_name", { name });
  },

  release(context: RequestContext, name: NameId) {
    return submitAndFinalize("dotns", context, "dotns", "release", { name });
  },

  removeExpiredName(context: RequestContext, name: NameId) {
    return submitAndFinalize("dotns", context, "dotns", "remove_expired_name", { name });
  },

  reserveName(
    context: RequestContext,
    parent: NameId | null,
    label: NormalizedLabel,
    beneficiary: AccountId | null,
    expires_at: BlockNumber | null,
  ) {
    return submitAndFinalize("dotns", context, "dotns", "reserve_name", {
      parent,
      label,
      beneficiary,
      expires_at,
    });
  },

  clearReservation(context: RequestContext, name: NameId) {
    return submitAndFinalize("dotns", context, "dotns", "clear_reservation", { name });
  },

  setLabelProtection(context: RequestContext, label: NormalizedLabel, protected_label: boolean) {
    return submitAndFinalize("dotns", context, "dotns", "set_label_protection", {
      label,
      protected: protected_label,
    });
  },

  setPaused(context: RequestContext, paused: boolean) {
    return submitAndFinalize("dotns", context, "dotns", "set_paused", { paused });
  },

  forceTransfer(context: RequestContext, name: NameId, new_owner: AccountId) {
    return submitAndFinalize("dotns", context, "dotns", "force_transfer", { name, new_owner });
  },

  forceRevoke(context: RequestContext, name: NameId) {
    return submitAndFinalize("dotns", context, "dotns", "force_revoke", { name });
  },

  setRegistrar(context: RequestContext, registrar: AccountId, enabled: boolean) {
    return submitAndFinalize("dotns", context, "dotns", "set_registrar", { registrar, enabled });
  },
} as const;

export type NameByIdResponse = Versioned<NameView>;
export type ResolvedAddress = Versioned<DotnsAddress>;
export type ResolvedSubject = Versioned<SubjectId>;
export type ResolvedAttestation = Versioned<AttestationId>;
export type ResolvedContent = Versioned<ContentCommitment>;
export type ResolvedText = Versioned<TextValue>;
