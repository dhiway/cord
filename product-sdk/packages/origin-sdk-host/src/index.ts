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


import { OriginSdkError, asSdkError, type SdkResult } from "@cord-network/origin-sdk-errors";
import { err } from "@cord-network/origin-sdk-result";
import type {
  HostAccount,
  HostCapability,
  HostFinalizedBlock,
  HostRuntimeIdentity,
  HostSignRequest,
  HostStatementDraft,
  HostStatementQuery,
  HostStatementRecord,
  OriginHostBridge,
  PermissionGrant,
  ProductIdentity,
} from "./protocol.ts";

export * from "./protocol.ts";

export interface OriginHostClient {
  readonly product: ProductIdentity;
  authorize(capability: HostCapability, signal?: AbortSignal): Promise<SdkResult<PermissionGrant>>;
  accounts(signal?: AbortSignal): Promise<SdkResult<readonly HostAccount[]>>;
  finalizedBlock(signal?: AbortSignal): Promise<SdkResult<HostFinalizedBlock>>;
  runtimeIdentity(at: `0x${string}`, signal?: AbortSignal): Promise<SdkResult<HostRuntimeIdentity>>;
  disconnectChain(signal?: AbortSignal): Promise<SdkResult<void>>;
  sign(request: HostSignRequest, signal?: AbortSignal): Promise<SdkResult<Uint8Array>>;
  getLocal(key: string, signal?: AbortSignal): Promise<SdkResult<Uint8Array | undefined>>;
  setLocal(key: string, value: Uint8Array, signal?: AbortSignal): Promise<SdkResult<void>>;
  deleteLocal(key: string, signal?: AbortSignal): Promise<SdkResult<void>>;
  submitStatement(draft: HostStatementDraft, signal?: AbortSignal): Promise<SdkResult<HostStatementRecord>>;
  queryStatements(query: HostStatementQuery, signal?: AbortSignal): Promise<SdkResult<readonly HostStatementRecord[]>>;
  subscribeStatements(
    query: HostStatementQuery,
    signal?: AbortSignal,
  ): AsyncIterable<SdkResult<readonly HostStatementRecord[]>>;
}

const validateProduct = (product: ProductIdentity): void => {
  if (!/^[a-z0-9][a-z0-9._-]{2,63}$/.test(product.id) || product.name.trim().length === 0) {
    throw new OriginSdkError({
      source: "host",
      domain: "product",
      code: "invalid_product",
      message: "Product identity is invalid",
    });
  }
};

function hostError(
  domain: string,
  code: string,
  message: string,
  retryable = false,
): SdkResult<never> {
  return err(new OriginSdkError({ source: "host", domain, code, message, retryable }));
}

function validateStorageKey(key: string): SdkResult<never> | undefined {
  if (key.length < 1 || key.length > 256 || key.includes("\0")) {
    return hostError("local-storage", "invalid_key", "Local storage key must contain 1-256 characters");
  }
  return undefined;
}

function cloneStatement(record: HostStatementRecord): HostStatementRecord {
  return { ...record, topics: [...record.topics], data: record.data.slice() };
}

export function createHostClient(
  bridge: OriginHostBridge,
  product: ProductIdentity,
  options: { readonly now?: () => number } = {},
): OriginHostClient {
  validateProduct(product);
  const now = options.now ?? Date.now;
  const invoke = async <T>(
    operation: () => Promise<SdkResult<T>>,
    signal?: AbortSignal,
  ): Promise<SdkResult<T>> => {
    if (signal?.aborted) return hostError("transport", "cancelled", "Host operation cancelled");
    try {
      const result = await operation();
      if (signal?.aborted) return hostError("transport", "cancelled", "Host operation cancelled");
      return result;
    } catch (error) {
      return err(asSdkError(error, {
        source: "host",
        domain: "transport",
        code: signal?.aborted ? "cancelled" : "host_unavailable",
        retryable: !signal?.aborted,
      }));
    }
  };
  const authorize = (capability: HostCapability, signal?: AbortSignal) => invoke(
    () => bridge.request(product, "permissions.authorize", { capability }, signal), signal,
  );
  const requireGrant = async (
    capability: HostCapability,
    signal?: AbortSignal,
  ): Promise<SdkResult<PermissionGrant>> => {
    const result = await authorize(capability, signal);
    if (!result.success) return result;
    if (result.value.productId !== product.id || result.value.capability !== capability) {
      return hostError("permission", "invalid_grant", "Host permission grant does not match the request");
    }
    if (result.value.expiresAt !== undefined && result.value.expiresAt <= now()) {
      return hostError("permission", "permission_expired", "Host permission grant has expired");
    }
    return result;
  };
  const granted = async <T>(
    capability: HostCapability,
    operation: () => Promise<SdkResult<T>>,
    signal?: AbortSignal,
  ): Promise<SdkResult<T>> => {
    const grant = await requireGrant(capability, signal);
    return grant.success ? invoke(operation, signal) : grant;
  };

  return {
    product: { ...product },
    authorize: requireGrant,
    accounts: (signal) => granted(
      "accounts", () => bridge.request(product, "accounts.list", {}, signal), signal,
    ),
    finalizedBlock: (signal) => granted(
      "chain", () => bridge.request(product, "chain.finalized-block", {}, signal), signal,
    ),
    runtimeIdentity: (at, signal) => granted(
      "chain", () => bridge.request(product, "chain.runtime-identity", { at }, signal), signal,
    ),
    disconnectChain: (signal) => granted(
      "chain", () => bridge.request(product, "chain.disconnect", {}, signal), signal,
    ),
    async sign(request, signal) {
      if (!(request.payload instanceof Uint8Array) || request.payload.length === 0) {
        return hostError("signing", "invalid_request", "Signing payload must not be empty");
      }
      if (!request.account || !request.purpose.trim()) {
        return hostError("signing", "invalid_request", "Signing account and purpose are required");
      }
      const result = await granted(
        "signing",
        () => bridge.request(product, "signing.approve", {
          ...request, payload: request.payload.slice(),
        }, signal),
        signal,
      );
      return result.success ? { success: true, value: result.value.slice() } : result;
    },
    async getLocal(key, signal) {
      const invalid = validateStorageKey(key);
      if (invalid) return invalid;
      const result = await granted(
        "local-storage", () => bridge.request(product, "local-storage.get", { key }, signal), signal,
      );
      return result.success
        ? { success: true, value: result.value?.slice() }
        : result;
    },
    async setLocal(key, value, signal) {
      const invalid = validateStorageKey(key);
      if (invalid) return invalid;
      if (!(value instanceof Uint8Array)) {
        return hostError("local-storage", "invalid_value", "Local storage value must be bytes");
      }
      return granted(
        "local-storage",
        () => bridge.request(product, "local-storage.set", { key, value: value.slice() }, signal),
        signal,
      );
    },
    async deleteLocal(key, signal) {
      const invalid = validateStorageKey(key);
      if (invalid) return invalid;
      return granted(
        "local-storage", () => bridge.request(product, "local-storage.delete", { key }, signal), signal,
      );
    },
    async submitStatement(draft, signal) {
      const result = await granted(
        "statements",
        () => bridge.request(product, "statements.submit", {
          ...draft, topics: [...draft.topics], data: draft.data.slice(),
        }, signal),
        signal,
      );
      return result.success ? { success: true, value: cloneStatement(result.value) } : result;
    },
    async queryStatements(query, signal) {
      const result = await granted(
        "statements",
        () => bridge.request(product, "statements.query", { ...query, topics: [...query.topics] }, signal),
        signal,
      );
      return result.success
        ? { success: true, value: result.value.map(cloneStatement) }
        : result;
    },
    async *subscribeStatements(query, signal = new AbortController().signal) {
      const grant = await requireGrant("statements", signal);
      if (!grant.success) {
        yield grant;
        return;
      }
      try {
        for await (const result of bridge.subscribe(
          product, "statements.subscribe", { ...query, topics: [...query.topics] }, signal,
        )) {
          if (signal.aborted) {
            yield hostError("statements", "cancelled", "Statement subscription cancelled");
            return;
          }
          yield result.success
            ? { success: true, value: result.value.map(cloneStatement) }
            : result;
        }
      } catch (error) {
        yield err(asSdkError(error, {
          source: "host",
          domain: "statements",
          code: signal.aborted ? "cancelled" : "subscription_failed",
          retryable: !signal.aborted,
        }));
      }
    },
  };
}

export interface HostChainProvider {
  finalizedBlock(signal?: AbortSignal): Promise<HostFinalizedBlock>;
  runtimeIdentity(at: `0x${string}`, signal?: AbortSignal): Promise<HostRuntimeIdentity>;
  disconnect(): Promise<void>;
}

function requireHostValue<T>(result: SdkResult<T>): T {
  if (result.success) return result.value;
  throw new OriginSdkError({
    source: result.error.source,
    domain: result.error.domain,
    code: result.error.code,
    message: result.error.message,
    retryable: result.error.retryable,
    ...(result.error.details === undefined ? {} : { details: result.error.details }),
  });
}

/** Structurally compatible provider for `createCommonsChainClient`; endpoint choice stays host-owned. */
export function createHostChainProvider(host: OriginHostClient): HostChainProvider {
  return {
    finalizedBlock: async (signal) => requireHostValue(await host.finalizedBlock(signal)),
    runtimeIdentity: async (at, signal) => requireHostValue(await host.runtimeIdentity(at, signal)),
    async disconnect() { requireHostValue(await host.disconnectChain()); },
  };
}
