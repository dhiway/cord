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

import type { Result } from "@cord-network/origin-sdk-result";

export const SDK_ERROR_MARKER = "cord.origin-sdk.error.v1" as const;

export interface SdkError {
  readonly marker: typeof SDK_ERROR_MARKER;
  readonly source: string;
  readonly domain: string;
  readonly code: string;
  readonly message: string;
  readonly retryable: boolean;
  readonly details?: Readonly<Record<string, unknown>>;
}

export type SdkResult<T> = Result<T, SdkError>;

export interface SdkErrorOptions {
  source: string;
  domain: string;
  code: string;
  message: string;
  retryable?: boolean;
  details?: Readonly<Record<string, unknown>>;
  cause?: unknown;
}

export class OriginSdkError extends Error implements SdkError {
  readonly marker = SDK_ERROR_MARKER;
  readonly source: string;
  readonly domain: string;
  readonly code: string;
  readonly retryable: boolean;
  readonly details?: Readonly<Record<string, unknown>>;

  constructor(options: SdkErrorOptions) {
    super(options.message, options.cause === undefined ? undefined : { cause: options.cause });
    this.name = "OriginSdkError";
    this.source = options.source;
    this.domain = options.domain;
    this.code = options.code;
    this.retryable = options.retryable ?? false;
    if (options.details !== undefined) this.details = options.details;
  }

  toJSON(): SdkError {
    return {
      marker: this.marker,
      source: this.source,
      domain: this.domain,
      code: this.code,
      message: this.message,
      retryable: this.retryable,
      ...(this.details === undefined ? {} : { details: this.details }),
    };
  }
}

export function isSdkError(value: unknown): value is SdkError {
  if (typeof value !== "object" || value === null) return false;
  const candidate = value as Partial<SdkError>;
  return candidate.marker === SDK_ERROR_MARKER
    && typeof candidate.source === "string"
    && typeof candidate.domain === "string"
    && typeof candidate.code === "string"
    && typeof candidate.message === "string"
    && typeof candidate.retryable === "boolean";
}

export function asSdkError(
  value: unknown,
  fallback: Omit<SdkErrorOptions, "message" | "cause"> & { message?: string },
): SdkError {
  if (isSdkError(value)) return value;
  return new OriginSdkError({
    ...fallback,
    message: fallback.message ?? (value instanceof Error ? value.message : "Unknown SDK failure"),
    cause: value,
  });
}
