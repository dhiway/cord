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

export type JsonValue = null | boolean | number | string | JsonValue[] | JsonObject;
export type JsonObject = { [key: string]: JsonValue };

export type NativeDomain = "identity" | "attestation" | "content" | "names" | "storage"
  | "provider" | "drive" | "s3" | "transaction";

export const DOMAIN_ERROR_CODES = [
  "invalid_input",
  "not_authorized",
  "not_found",
  "expired",
  "conflict",
  "capacity_exceeded",
  "proof_invalid",
  "content_unavailable",
  "content_integrity",
  "runtime_rejected",
] as const satisfies readonly ErrorCode[];

export type DomainErrorCode = (typeof DOMAIN_ERROR_CODES)[number];

export type ErrorCode =
  | "invalid_input"
  | "not_authorized"
  | "not_found"
  | "expired"
  | "conflict"
  | "capacity_exceeded"
  | "proof_invalid"
  | "content_unavailable"
  | "content_integrity"
  | "runtime_rejected";

/** Native error v1 with an explicit domain and semantic operation. */
export class NativeDomainError extends Error {
  readonly version = 1;
  readonly code: DomainErrorCode;
  readonly retryable: boolean;
  readonly details: JsonObject;
  readonly domain: NativeDomain;
  readonly operation: string;

  constructor(
    domain: NativeDomain,
    operation: string,
    code: DomainErrorCode,
    message: string,
    retryable = false,
    details: JsonObject = {},
  ) {
    super(message);
    this.name = "NativeDomainError";
    this.code = code;
    this.retryable = retryable;
    this.details = { ...details, domain, operation };
    this.domain = domain;
    this.operation = operation;
  }

  toJSON(): JsonObject {
    return {
      version: this.version,
      code: this.code,
      message: this.message,
      retryable: this.retryable,
      details: this.details,
    };
  }
}

export function invalidDomainInput(
  domain: NativeDomain,
  operation: string,
  message: string,
): never {
  throw new NativeDomainError(domain, operation, "invalid_input", message);
}
