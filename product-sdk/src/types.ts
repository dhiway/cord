/** Opaque, JSON-safe identifiers used by the native product surfaces. */
declare const nativeType: unique symbol;
export type NativeString<Kind extends string> = string & { readonly [nativeType]: Kind };

export type AccountId = NativeString<"AccountId">;
export type Hash32 = NativeString<"Hash32">;
export type SchemaId = NativeString<"SchemaId">;
export type SubjectId = NativeString<"SubjectId">;
export type AttestationId = NativeString<"AttestationId">;
export type SubjectCommitment = NativeString<"SubjectCommitment">;
export type PayloadCommitment = NativeString<"PayloadCommitment">;
export type StatusCommitment = NativeString<"StatusCommitment">;
export type UniquenessCommitment = NativeString<"UniquenessCommitment">;
export type NameId = NativeString<"NameId">;
export type RegistrationCommitment = NativeString<"RegistrationCommitment">;
export type RegistrationSalt = NativeString<"RegistrationSalt">;
export type ContentCommitment = NativeString<"ContentCommitment">;
export type ContentHash = NativeString<"ContentHash">;
export type ReservationId = NativeString<"ReservationId">;
export type ProviderId = NativeString<"ProviderId">;
export type ProviderAllocationId = NativeString<"ProviderAllocationId">;
export type AgreementId = NativeString<"AgreementId">;
export type ChallengeId = NativeString<"ChallengeId">;
export type ContainerId = NativeString<"ContainerId">;
export type DriveId = NativeString<"DriveId">;
export type BucketId = NativeString<"BucketId">;
export type ObjectId = NativeString<"ObjectId">;
export type BlockHash = NativeString<"BlockHash">;
export type IssuerSignature = NativeString<"IssuerSignature">;
export type DecimalU64 = NativeString<"DecimalU64">;
export type BlockNumber = NativeString<"BlockNumber">;

export const MAX_PAGE_SIZE = 100;
const HASH_32 = /^0x[0-9a-fA-F]{64}$/;
const DECIMAL = /^(0|[1-9][0-9]*)$/;

export function nativeHash<Kind extends string>(value: string, label: string): NativeString<Kind> {
  if (!HASH_32.test(value)) throw new TypeError(`${label} must be a 32-byte 0x-prefixed hash`);
  return value.toLowerCase() as NativeString<Kind>;
}

export function nativeText<Kind extends string>(
  value: string,
  label: string,
  maxLength = 128,
): NativeString<Kind> {
  if (!value || value.length > maxLength) throw new TypeError(`${label} must contain 1-${maxLength} characters`);
  return value as NativeString<Kind>;
}

export function decimalU64<Kind extends "DecimalU64" | "BlockNumber" | "ReservationId">(
  value: string | number,
  label: string,
): NativeString<Kind> {
  const text = String(value);
  if (!DECIMAL.test(text) || BigInt(text) > 18_446_744_073_709_551_615n) {
    throw new TypeError(`${label} must be an unsigned 64-bit decimal string`);
  }
  return text as NativeString<Kind>;
}

/** Construct the exact Orbis Storage reservation identifier used by provider agreements. */
export function reservationId(value: string | number): ReservationId {
  return decimalU64<"ReservationId">(value, "reservation_id");
}

export interface PageInput {
  readonly cursor?: number | null;
  readonly limit?: number;
}

export interface PageRequest {
  readonly cursor: number | null;
  readonly limit: number;
}

/** Normalize every product-domain page to the runtime API's common hard cap. */
export function page(input: PageInput = {}): PageRequest {
  const cursor = input.cursor ?? null;
  if (cursor !== null && (!Number.isSafeInteger(cursor) || cursor < 0 || cursor > 0xffff_ffff)) {
    throw new TypeError("cursor must be a non-negative u32 offset");
  }
  const requested = input.limit ?? 50;
	if (!Number.isSafeInteger(requested) || requested < 0) {
		throw new TypeError("limit must be a non-negative integer");
  }
  return { cursor, limit: Math.min(requested, MAX_PAGE_SIZE) };
}

export interface Versioned<T> {
  readonly version: 1;
  readonly value: T | null;
}

/** Host evidence that a decoded native response came from one finalized state. */
export interface FinalizedResponse<T> {
  readonly finalized_hash: BlockHash;
  readonly response: T;
}

export interface IdPage<Id> {
  readonly version: 1;
  readonly items: readonly Id[];
  readonly next_cursor: number | null;
  readonly finalized_hash: BlockHash;
}
