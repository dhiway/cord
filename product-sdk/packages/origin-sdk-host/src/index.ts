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
import { err, ok } from "@cord-network/origin-sdk-result";

export type HostCapability = "accounts" | "signing" | "local-storage" | "preimages" | "statements";
export interface ProductIdentity { readonly id: string; readonly name: string; }
export interface HostAccount { readonly address: string; readonly name?: string; }
export interface PermissionGrant { readonly productId: string; readonly capability: HostCapability; readonly expiresAt?: number; }
export interface HostSignRequest { readonly account: string; readonly payload: Uint8Array; readonly purpose: string; }

export interface OriginHostBridge {
  authorize(product: ProductIdentity, capability: HostCapability, signal?: AbortSignal): Promise<SdkResult<PermissionGrant>>;
  accounts(product: ProductIdentity, signal?: AbortSignal): Promise<SdkResult<readonly HostAccount[]>>;
  approveAndSign(product: ProductIdentity, request: HostSignRequest, signal?: AbortSignal): Promise<SdkResult<Uint8Array>>;
  getStorage(product: ProductIdentity, key: string, signal?: AbortSignal): Promise<SdkResult<Uint8Array | undefined>>;
  setStorage(product: ProductIdentity, key: string, value: Uint8Array, signal?: AbortSignal): Promise<SdkResult<void>>;
}

export interface OriginHostClient {
  readonly product: ProductIdentity;
  authorize(capability: HostCapability, signal?: AbortSignal): Promise<SdkResult<PermissionGrant>>;
  accounts(signal?: AbortSignal): Promise<SdkResult<readonly HostAccount[]>>;
  sign(request: HostSignRequest, signal?: AbortSignal): Promise<SdkResult<Uint8Array>>;
  getLocal(key: string, signal?: AbortSignal): Promise<SdkResult<Uint8Array | undefined>>;
  setLocal(key: string, value: Uint8Array, signal?: AbortSignal): Promise<SdkResult<void>>;
}

const validateProduct = (product: ProductIdentity): void => {
  if (!/^[a-z0-9][a-z0-9._-]{2,63}$/.test(product.id) || product.name.trim().length === 0)
    throw new OriginSdkError({ source: "host", domain: "product", code: "invalid_product", message: "Product identity is invalid" });
};

export function createHostClient(
  bridge: OriginHostBridge,
  product: ProductIdentity,
  options: { readonly now?: () => number } = {},
): OriginHostClient {
  validateProduct(product);
  const now = options.now ?? Date.now;
  const invoke = async <T>(operation: () => Promise<SdkResult<T>>): Promise<SdkResult<T>> => {
    try { return await operation(); }
    catch (error) { return err(asSdkError(error, { source: "host", domain: "transport", code: "host_unavailable", retryable: true })); }
  };
  const requireGrant = async (capability: HostCapability, signal?: AbortSignal): Promise<SdkResult<PermissionGrant>> => {
    const result = await invoke(() => bridge.authorize(product, capability, signal));
    if (!result.success) return result;
    if (result.value.productId !== product.id || result.value.capability !== capability
      || (result.value.expiresAt !== undefined && result.value.expiresAt <= now()))
      return err(new OriginSdkError({ source: "host", domain: "permission", code: "invalid_grant", message: "Host permission grant is invalid" }));
    return result;
  };
  return {
    product,
    authorize: requireGrant,
    async accounts(signal) { const grant=await requireGrant("accounts",signal); return grant.success?invoke(()=>bridge.accounts(product,signal)):grant; },
    async sign(request,signal) { const grant=await requireGrant("signing",signal); return grant.success?invoke(()=>bridge.approveAndSign(product,request,signal)):grant; },
    async getLocal(key,signal) { const grant=await requireGrant("local-storage",signal); return grant.success?invoke(()=>bridge.getStorage(product,key,signal)):grant; },
    async setLocal(key,value,signal) { const grant=await requireGrant("local-storage",signal); return grant.success?invoke(()=>bridge.setStorage(product,key,value,signal)):grant; },
  };
}
