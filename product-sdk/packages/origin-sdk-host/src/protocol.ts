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
import { err } from "@cord-network/origin-sdk-result";

export const TRUAPI_PROTOCOL = "cord.origin-host" as const;
export const TRUAPI_VERSION = 1 as const;

export type HostCapability =
  | "accounts"
  | "chain"
  | "signing"
  | "local-storage"
  | "preimages"
  | "resources"
  | "statements";

export interface ProductIdentity {
  readonly id: string;
  readonly name: string;
}

export interface HostAccount {
  readonly address: string;
  readonly name?: string;
}

export interface PermissionGrant {
  readonly productId: string;
  readonly capability: HostCapability;
  readonly expiresAt?: number;
}

export interface HostSignRequest {
  readonly account: string;
  readonly payload: Uint8Array;
  readonly purpose: string;
}

export interface HostFinalizedBlock {
  readonly hash: `0x${string}`;
  readonly number: bigint;
}

export interface HostRuntimeIdentity {
  readonly genesis_hash: `0x${string}`;
  readonly spec_version: number;
  readonly transaction_version: number;
  readonly metadata_hash: `0x${string}`;
  readonly descriptor_contract_sha256: string;
  readonly chain_spec_source_sha256: string;
}

export interface HostPreimageReference {
  readonly contentHash: `0x${string}`;
  readonly size: number;
  readonly contentType?: string;
}

export type HostResourceRequest =
  | {
    readonly kind: "statement-allowance";
    readonly account: string;
    readonly bytes: bigint;
    readonly expiresAt?: bigint;
  }
  | {
    readonly kind: "storage-reservation";
    readonly account: string;
    readonly bytes: bigint;
    readonly expiresAt?: bigint;
  };

export interface HostResourceGrant {
  readonly kind: HostResourceRequest["kind"];
  readonly account: string;
  readonly authorization: Uint8Array;
  readonly expiresAt?: bigint;
}

export interface HostStatementDraft {
  readonly account: string;
  readonly topics: readonly string[];
  readonly data: Uint8Array;
  readonly destination?: string;
  readonly channel?: string;
  readonly priority?: number;
}

export type HostStatementQuery =
  | { readonly kind: "broadcasts"; readonly topics: readonly string[] }
  | { readonly kind: "posted" | "posted-clear"; readonly topics: readonly string[]; readonly destination: string };

export interface HostStatementRecord extends HostStatementDraft {
  readonly hash: string;
}

export interface HostMethodMap {
  "permissions.authorize": {
    readonly input: { readonly capability: HostCapability };
    readonly output: PermissionGrant;
  };
  "accounts.list": { readonly input: Readonly<Record<string, never>>; readonly output: readonly HostAccount[] };
  "chain.finalized-block": { readonly input: Readonly<Record<string, never>>; readonly output: HostFinalizedBlock };
  "chain.runtime-identity": { readonly input: { readonly at: `0x${string}` }; readonly output: HostRuntimeIdentity };
  "chain.disconnect": { readonly input: Readonly<Record<string, never>>; readonly output: void };
  "signing.approve": { readonly input: HostSignRequest; readonly output: Uint8Array };
  "local-storage.get": { readonly input: { readonly key: string }; readonly output: Uint8Array | undefined };
  "local-storage.set": { readonly input: { readonly key: string; readonly value: Uint8Array }; readonly output: void };
  "local-storage.delete": { readonly input: { readonly key: string }; readonly output: void };
  "preimages.put": {
    readonly input: { readonly bytes: Uint8Array; readonly contentType?: string };
    readonly output: HostPreimageReference;
  };
  "preimages.get": { readonly input: { readonly reference: HostPreimageReference }; readonly output: Uint8Array };
  "resources.allocate": { readonly input: HostResourceRequest; readonly output: HostResourceGrant };
  "statements.submit": { readonly input: HostStatementDraft; readonly output: HostStatementRecord };
  "statements.query": { readonly input: HostStatementQuery; readonly output: readonly HostStatementRecord[] };
}

export interface HostSubscriptionMap {
  "statements.subscribe": { readonly input: HostStatementQuery; readonly output: readonly HostStatementRecord[] };
}

export type HostMethod = keyof HostMethodMap;
export type HostSubscriptionMethod = keyof HostSubscriptionMap;
type HostProtocolCapability = HostCapability | "permissions";

export interface TruApiRequest {
  readonly protocol: typeof TRUAPI_PROTOCOL;
  readonly version: typeof TRUAPI_VERSION;
  readonly requestId: string;
  readonly product: ProductIdentity;
  readonly capability: HostProtocolCapability;
  readonly operation: string;
  readonly payload: unknown;
}

export interface TruApiResponse {
  readonly protocol: typeof TRUAPI_PROTOCOL;
  readonly version: typeof TRUAPI_VERSION;
  readonly requestId: string;
  readonly value: unknown;
}

export interface TruApiTransport {
  request(request: TruApiRequest, signal?: AbortSignal): Promise<SdkResult<TruApiResponse>>;
  subscribe?(
    request: TruApiRequest,
    signal: AbortSignal,
  ): AsyncIterable<SdkResult<TruApiResponse>>;
}

export interface OriginHostBridge {
  request<Method extends HostMethod>(
    product: ProductIdentity,
    method: Method,
    input: HostMethodMap[Method]["input"],
    signal?: AbortSignal,
  ): Promise<SdkResult<HostMethodMap[Method]["output"]>>;
  subscribe<Method extends HostSubscriptionMethod>(
    product: ProductIdentity,
    method: Method,
    input: HostSubscriptionMap[Method]["input"],
    signal: AbortSignal,
  ): AsyncIterable<SdkResult<HostSubscriptionMap[Method]["output"]>>;
}

export interface TruApiBridgeOptions {
  readonly requestId?: () => string;
}

let requestSequence = 0;
const defaultRequestId = (): string => `origin-host-${++requestSequence}`;

function splitMethod(method: string): { capability: HostProtocolCapability; operation: string } {
  const separator = method.indexOf(".");
  return {
    capability: method.slice(0, separator) as HostProtocolCapability,
    operation: method.slice(separator + 1),
  };
}

function protocolError(code: string, message: string): SdkResult<never> {
  return err(new OriginSdkError({
    source: "host",
    domain: "protocol",
    code,
    message,
    retryable: code === "host_unavailable",
  }));
}

function validateResponse(
  response: TruApiResponse,
  requestId: string,
): SdkResult<unknown> {
  if (response.protocol !== TRUAPI_PROTOCOL || response.version !== TRUAPI_VERSION) {
    return protocolError("unsupported_protocol", "Host response uses an unsupported protocol version");
  }
  if (response.requestId !== requestId) {
    return protocolError("response_mismatch", "Host response request ID does not match");
  }
  return { success: true, value: response.value };
}

export function createTruApiBridge(
  transport: TruApiTransport,
  options: TruApiBridgeOptions = {},
): OriginHostBridge {
  const nextRequestId = options.requestId ?? defaultRequestId;
  const envelope = (product: ProductIdentity, method: string, payload: unknown): TruApiRequest => {
    const { capability, operation } = splitMethod(method);
    return {
      protocol: TRUAPI_PROTOCOL,
      version: TRUAPI_VERSION,
      requestId: nextRequestId(),
      product: { ...product },
      capability,
      operation,
      payload,
    };
  };
  return {
    async request(product, method, input, signal) {
      const request = envelope(product, method, input);
      const response = await transport.request(request, signal);
      if (!response.success) return response;
      return validateResponse(response.value, request.requestId) as SdkResult<
        HostMethodMap[typeof method]["output"]
      >;
    },
    async *subscribe(product, method, input, signal) {
      if (transport.subscribe === undefined) {
        yield protocolError("host_unavailable", "Host does not support subscriptions");
        return;
      }
      const request = envelope(product, method, input);
      for await (const response of transport.subscribe(request, signal)) {
        if (!response.success) {
          yield response;
          continue;
        }
        yield validateResponse(response.value, request.requestId) as SdkResult<
          HostSubscriptionMap[typeof method]["output"]
        >;
      }
    },
  };
}
