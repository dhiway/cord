import { invalidDomainInput } from "./errors.ts";
import { finalizedRead, submitAndFinalize, type RequestContext } from "./host.ts";
import type {
  AccountId,
  BlockNumber,
  ContentHash,
  DecimalU64,
  ProviderAllocationId,
  ReservationId,
} from "./types.ts";

declare const contentType: unique symbol;
export type Base64Content = string & { readonly [contentType]: "Base64Content" };
export type HashingAlgorithm = "blake2b256" | "sha2_256" | "keccak256";

export interface CidConfig {
  /** Unsigned multicodec value encoded as an exact decimal string. */
  readonly codec: DecimalU64;
  readonly hashing: HashingAlgorithm;
}

export interface BulletinRef {
  readonly block: BlockNumber;
  readonly transaction_index: number;
}

export type TransactionRef =
  | { readonly kind: "position"; readonly block: BlockNumber; readonly index: number }
  | { readonly kind: "content_hash"; readonly content_hash: ContentHash };

export interface AccountAuthorization {
  readonly expires_at: BlockNumber;
  readonly bytes_allowance: DecimalU64;
  readonly bytes_used: DecimalU64;
  readonly bytes_permanent_used: DecimalU64;
  readonly transactions_allowance: number;
  readonly transactions_used: number;
}

export type StorageActor =
  | { readonly kind: "account"; readonly account: AccountId }
  | { readonly kind: "root" }
  | { readonly kind: "preimage"; readonly content_hash: ContentHash }
  | { readonly kind: "auto_renew"; readonly account: AccountId };

/** Current actor provenance, or `null` when the exact retained position is absent. */
export type StoredContentProvenance = StorageActor | null;

export interface ResourceReservationLink {
  readonly reservation_id: ReservationId;
  readonly content_hash: ContentHash;
  readonly bulletin_ref: BulletinRef;
  readonly owner: AccountId;
  readonly size: number;
  readonly retention_boundary: BlockNumber;
}

export function base64Content(value: string): Base64Content {
  if (!value || value.length % 4 !== 0 || !/^[A-Za-z0-9+/]*={0,2}$/.test(value)) {
    invalidDomainInput("storage", "content", "content must be non-empty canonical base64");
  }
  return value as Base64Content;
}

export const storage = {
  accountAuthorization(context: RequestContext, account: AccountId) {
    return finalizedRead("storage", context, "storage", "account_authorization", { account });
  },

  canStore(context: RequestContext, account: AccountId, data_len: number) {
    return finalizedRead("storage", context, "storage", "can_store", { account, data_len });
  },

  canRenew(context: RequestContext, account: AccountId, entry: TransactionRef) {
    return finalizedRead("storage", context, "storage", "can_renew", { account, entry: { ...entry } });
  },

  storedContentProvenance(context: RequestContext, reference: BulletinRef) {
    return finalizedRead("storage", context, "storage", "stored_content_provenance", {
      reference: { ...reference },
    });
  },

  resourceReservation(context: RequestContext, reservation_id: ReservationId) {
    return finalizedRead("storage", context, "storage", "resource_reservation", { reservation_id });
  },

  resourceReservationLink(
    context: RequestContext,
    reservation_id: ReservationId,
    content_hash: ContentHash,
  ) {
    return finalizedRead("storage", context, "storage", "resource_reservation_link", {
      reservation_id,
      content_hash,
    });
  },

  resourceProviderRef(context: RequestContext, reservation_id: ReservationId) {
    return finalizedRead("storage", context, "storage", "resource_provider_ref", { reservation_id });
  },

  store(context: RequestContext, content_base64: Base64Content) {
    return submitAndFinalize("storage", context, "storage", "store", { content_base64 });
  },

  storeWithCidConfig(context: RequestContext, cid_config: CidConfig, content_base64: Base64Content) {
    return submitAndFinalize("storage", context, "storage", "store_with_cid_config", {
      cid_config: { ...cid_config },
      content_base64,
    });
  },

  storeReserved(
    context: RequestContext,
    reservation_id: ReservationId,
    cid_config: CidConfig,
    content_base64: Base64Content,
  ) {
    return submitAndFinalize("storage", context, "storage", "store_reserved", {
      reservation_id,
      cid_config: { ...cid_config },
      content_base64,
    });
  },

  renewReserved(
    context: RequestContext,
    reservation_id: ReservationId,
    content_hash: ContentHash,
  ) {
    return submitAndFinalize("storage", context, "storage", "renew_reserved", {
      reservation_id,
      content_hash,
    });
  },

  attachProvider(
    context: RequestContext,
    reservation_id: ReservationId,
    provider_ref: ProviderAllocationId,
  ) {
    return submitAndFinalize("storage", context, "storage", "attach_provider", {
      reservation_id,
      provider_ref,
    });
  },

  renew(context: RequestContext, entry: TransactionRef) {
    return submitAndFinalize("storage", context, "storage", "renew", { entry: { ...entry } });
  },

  forceRenew(context: RequestContext, entry: TransactionRef) {
    return submitAndFinalize("storage", context, "storage", "force_renew", { entry: { ...entry } });
  },

  enableAutoRenew(context: RequestContext, content_hash: ContentHash) {
    return submitAndFinalize("storage", context, "storage", "enable_auto_renew", { content_hash });
  },

  disableAutoRenew(context: RequestContext, content_hash: ContentHash) {
    return submitAndFinalize("storage", context, "storage", "disable_auto_renew", { content_hash });
  },
} as const;
