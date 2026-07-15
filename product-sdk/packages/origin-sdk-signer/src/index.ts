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
import type { HostAccount, OriginHostClient } from "@cord-network/origin-sdk-host";
import { err, ok } from "@cord-network/origin-sdk-result";

export interface SignRequest {
  readonly account: string;
  readonly payload: Uint8Array;
  readonly purpose: string;
}

export interface OriginSigner {
  accounts(signal?: AbortSignal): Promise<SdkResult<readonly HostAccount[]>>;
  sign(request: SignRequest, signal?: AbortSignal): Promise<SdkResult<Uint8Array>>;
}

export interface SelectedOriginSigner extends OriginSigner {
  readonly account: HostAccount;
}

function signerError(code: string, message: string): SdkResult<never> {
  return err(new OriginSdkError({
    source: "signer",
    domain: "account",
    code,
    message,
    retryable: false,
  }));
}

export function createHostSigner(host: OriginHostClient): OriginSigner {
  return {
    accounts: (signal) => host.accounts(signal),
    sign: (request, signal) => host.sign(request, signal),
  };
}

export function createSelectedHostSigner(
  host: OriginHostClient,
  account: HostAccount,
): SelectedOriginSigner {
  const selected = { ...account };
  return {
    account: selected,
    async accounts(signal) {
      const accounts = await host.accounts(signal);
      if (!accounts.success) return accounts;
      const current = accounts.value.find(({ address }) => address === selected.address);
      return current === undefined
        ? signerError("account_unavailable", "Selected host account is no longer available")
        : ok([{ ...current }]);
    },
    sign(request, signal) {
      if (request.account !== selected.address) {
        return Promise.resolve(signerError(
          "account_mismatch",
          "Signing request does not match the selected host account",
        ));
      }
      return host.sign(request, signal);
    },
  };
}

export async function selectHostSigner(
  host: OriginHostClient,
  address?: string,
  signal?: AbortSignal,
): Promise<SdkResult<SelectedOriginSigner>> {
  const accounts = await host.accounts(signal);
  if (!accounts.success) return accounts;
  const selected = address === undefined
    ? accounts.value[0]
    : accounts.value.find((account) => account.address === address);
  return selected === undefined
    ? signerError(address === undefined ? "no_accounts" : "account_unavailable", "Requested host account is unavailable")
    : ok(createSelectedHostSigner(host, selected));
}

export interface InjectedSignerProvider {
  accounts(signal?: AbortSignal): Promise<SdkResult<readonly HostAccount[]>>;
  approveAndSign(request: SignRequest, signal?: AbortSignal): Promise<SdkResult<Uint8Array>>;
}

export function createInjectedSigner(provider: InjectedSignerProvider): OriginSigner {
  return {
    async accounts(signal) {
      try { return await provider.accounts(signal); }
      catch (error) {
        return err(asSdkError(error, {
          source: "signer",
          domain: "accounts",
          code: signal?.aborted ? "cancelled" : "provider_unavailable",
          retryable: !signal?.aborted,
        }));
      }
    },
    async sign(request, signal) {
      try { return await provider.approveAndSign(request, signal); }
      catch (error) {
        return err(asSdkError(error, {
          source: "signer",
          domain: "approval",
          code: signal?.aborted ? "cancelled" : "signing_rejected",
          retryable: false,
        }));
      }
    },
  };
}
