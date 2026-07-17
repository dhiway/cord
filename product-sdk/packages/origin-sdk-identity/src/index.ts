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

declare const identityType: unique symbol;
export type AccountId = string & { readonly [identityType]: "AccountId" };
export type Hash32 = string & { readonly [identityType]: "Hash32" };

const HASH_32 = /^0x[0-9a-f]{64}$/;

export function accountId(value: string): AccountId {
  if (value.length < 1 || value.length > 128) throw new TypeError("account must contain 1-128 characters");
  return value as AccountId;
}

export function hash32(value: string): Hash32 {
  if (!HASH_32.test(value)) throw new TypeError("hash must be a lowercase 32-byte 0x-prefixed value");
  return value as Hash32;
}

export interface Versioned<T> {
  readonly version: 1;
  readonly value: T | null;
}

/**
 * Canonical Host-v2 Identity facade. The seven Identity operations use independent grants and
 * closed responses. Transaction signing uses its own operation and grant.
 */
export * from "./v2.ts";
