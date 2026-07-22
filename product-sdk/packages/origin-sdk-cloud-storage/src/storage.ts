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

import { invalidDomainInput, type AccountId, type BlockNumber, type ContentHash, type DecimalU64, type ProviderAllocationId, type ReservationId } from "./types.ts";

declare const contentType: unique symbol;
export type Base64Content = string & { readonly [contentType]: "Base64Content" };
export type HashingAlgorithm = "blake2b256" | "sha2_256" | "keccak256";

export interface CidConfig {
  /** Unsigned multicodec value encoded as an exact decimal string. */
  readonly codec: DecimalU64;
  readonly hashing: HashingAlgorithm;
}

export interface StorageRef {
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
  readonly storage_ref: StorageRef;
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
