import { finalizedRead, submitAndFinalize, type RequestContext } from "./host.ts";
import {
  page,
  type AccountId,
  type AttestationId,
  type BlockNumber,
  type ContentCommitment,
  type NameId,
  type PageInput,
  type RegistrationCommitment,
  type RegistrationSalt,
  type SubjectId,
  type Versioned,
  nativeText,
} from "./types.ts";
import { invalidDomainInput } from "./errors.ts";

declare const dotnsType: unique symbol;
export type NormalizedLabel = string & { readonly [dotnsType]: "NormalizedLabel" };
export type DotnsAddress = string & { readonly [dotnsType]: "DotnsAddress" };
export type TextKey = string & { readonly [dotnsType]: "TextKey" };
export type TextValue = string & { readonly [dotnsType]: "TextValue" };

const utf8 = new TextEncoder();

export function normalizedLabel(value: string): NormalizedLabel {
  if (value.length > 63 || !/^[a-z0-9](?:[a-z0-9-]*[a-z0-9])?$/.test(value)) {
    invalidDomainInput("dotns", "normalize_label", "label must follow lowercase ASCII label policy v1");
  }
  return value as NormalizedLabel;
}

export function registrationSalt(value: string): RegistrationSalt {
  return nativeText<"RegistrationSalt">(value, "registration salt", 64);
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
  if (utf8.encode(value).length > 256) {
    invalidDomainInput("dotns", "text_value", "text value must contain at most 256 UTF-8 bytes");
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

export const dotns = {
  nameById(context: RequestContext, name: NameId) {
    return finalizedRead("dotns", context, "dotns", "name_by_id", { name });
  },

  rootNameByNormalizedLabel(context: RequestContext, label: NormalizedLabel) {
    return finalizedRead("dotns", context, "dotns", "root_name_by_normalized_label", { label });
  },

  ownerNames(context: RequestContext, owner: AccountId, input?: PageInput) {
    return finalizedRead("dotns", context, "dotns", "owner_names", { owner, ...page(input) });
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
} as const;

export type NameByIdResponse = Versioned<NameView>;
export type ResolvedAddress = Versioned<DotnsAddress>;
export type ResolvedSubject = Versioned<SubjectId>;
export type ResolvedAttestation = Versioned<AttestationId>;
export type ResolvedContent = Versioned<ContentCommitment>;
export type ResolvedText = Versioned<TextValue>;
