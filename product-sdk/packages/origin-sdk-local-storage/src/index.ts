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


import { OriginSdkError, type SdkResult } from "@cord-network/origin-sdk-errors";
import type { OriginHostClient } from "@cord-network/origin-sdk-host";
import { err, ok } from "@cord-network/origin-sdk-result";

export interface StorageCodec<Value> {
  encode(value: Value): Uint8Array;
  decode(bytes: Uint8Array): Value;
}

export interface LocalStorageClient {
  readonly namespace: string;
  get<Value>(
    key: string,
    codec: StorageCodec<Value>,
    signal?: AbortSignal,
  ): Promise<SdkResult<Value | undefined>>;
  set<Value>(
    key: string,
    value: Value,
    codec: StorageCodec<Value>,
    signal?: AbortSignal,
  ): Promise<SdkResult<void>>;
  delete(key: string, signal?: AbortSignal): Promise<SdkResult<void>>;
}

const textEncoder = new TextEncoder();
const textDecoder = new TextDecoder("utf-8", { fatal: true });
const MAX_LOCAL_VALUE_BYTES = 5 * 1024 * 1024;

function storageError(code: string, message: string): SdkResult<never> {
  return err(new OriginSdkError({
    source: "local-storage",
    domain: "codec",
    code,
    message,
    retryable: false,
  }));
}

function validateSegment(value: string, label: string, max: number): void {
  if (!/^[a-zA-Z0-9][a-zA-Z0-9._/-]*$/.test(value) || value.length > max) {
    throw new TypeError(`${label} must contain 1-${max} portable key characters`);
  }
}

export const bytesCodec: StorageCodec<Uint8Array> = {
  encode: (value) => {
    if (!(value instanceof Uint8Array)) throw new TypeError("value must be bytes");
    return value.slice();
  },
  decode: (value) => value.slice(),
};

export const utf8Codec: StorageCodec<string> = {
  encode: (value) => textEncoder.encode(value),
  decode: (value) => textDecoder.decode(value),
};

export function jsonCodec<Value>(
  validate: (value: unknown) => value is Value,
): StorageCodec<Value> {
  return {
    encode(value) {
      const encoded = JSON.stringify(value);
      if (encoded === undefined) throw new TypeError("value is not JSON serializable");
      return textEncoder.encode(encoded);
    },
    decode(bytes) {
      const value: unknown = JSON.parse(textDecoder.decode(bytes));
      if (!validate(value)) throw new TypeError("stored JSON does not match the expected shape");
      return value;
    },
  };
}

export function createLocalStorage(
  host: OriginHostClient,
  namespace: string,
): LocalStorageClient {
  validateSegment(namespace, "storage namespace", 64);
  const scopedKey = (key: string): string => {
    validateSegment(key, "storage key", 128);
    return `${namespace}/${key}`;
  };
  return {
    namespace,
    async get(key, codec, signal) {
      let scoped: string;
      try { scoped = scopedKey(key); }
      catch (error) {
        return storageError("invalid_key", error instanceof Error ? error.message : "Invalid storage key");
      }
      const result = await host.getLocal(scoped, signal);
      if (!result.success) return result;
      if (result.value === undefined) return ok(undefined);
      try { return ok(codec.decode(result.value.slice())); }
      catch (error) {
        return storageError("decode_failed", error instanceof Error ? error.message : "Storage decode failed");
      }
    },
    async set(key, value, codec, signal) {
      let scoped: string;
      try { scoped = scopedKey(key); }
      catch (error) {
        return storageError("invalid_key", error instanceof Error ? error.message : "Invalid storage key");
      }
      let encoded: Uint8Array;
      try {
        encoded = codec.encode(value);
        if (!(encoded instanceof Uint8Array)) throw new TypeError("codec must return bytes");
        if (encoded.length > MAX_LOCAL_VALUE_BYTES) {
          throw new TypeError(`local storage value exceeds ${MAX_LOCAL_VALUE_BYTES} bytes`);
        }
      } catch (error) {
        return storageError("encode_failed", error instanceof Error ? error.message : "Storage encode failed");
      }
      return host.setLocal(scoped, encoded.slice(), signal);
    },
    async delete(key, signal) {
      let scoped: string;
      try { scoped = scopedKey(key); }
      catch (error) {
        return storageError("invalid_key", error instanceof Error ? error.message : "Invalid storage key");
      }
      return host.deleteLocal(scoped, signal);
    },
  };
}

export const LOCAL_STORAGE_CONTRACT = {
  maxValueBytes: MAX_LOCAL_VALUE_BYTES,
  hostCapability: "local-storage",
  directBrowserStorageFallback: false,
  productScopedByHost: true,
} as const;
