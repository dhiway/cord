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


import { OriginSdkError } from "@cord-network/origin-sdk-errors";
import type { HostAccount } from "@cord-network/origin-sdk-host";
import { err, ok } from "@cord-network/origin-sdk-result";
import type { OriginSigner, SignRequest } from "./index.ts";

export interface FakeSignerOptions {
  readonly accounts?: readonly HostAccount[];
  readonly signature?: Uint8Array;
}

export interface FakeSigner extends OriginSigner {
  readonly requests: readonly SignRequest[];
  setAccounts(accounts: readonly HostAccount[]): void;
  rejectNextSignature(): void;
  reset(): void;
}

const cloneRequest = (request: SignRequest): SignRequest => ({
  ...request,
  payload: request.payload.slice(),
});

export function createFakeSigner(options: FakeSignerOptions = {}): FakeSigner {
  const initialAccounts = options.accounts?.map((account) => ({ ...account })) ?? [];
  let accounts = initialAccounts.map((account) => ({ ...account }));
  let rejectNext = false;
  const requests: SignRequest[] = [];
  const signature = options.signature?.slice() ?? new Uint8Array(64);

  return {
    get requests() { return requests.map(cloneRequest); },
    async accounts() { return ok(accounts.map((account) => ({ ...account }))); },
    async sign(request, signal) {
      if (signal?.aborted) {
        return err(new OriginSdkError({
          source: "fake-signer",
          domain: "approval",
          code: "cancelled",
          message: "Fake signing request cancelled",
        }));
      }
      requests.push(cloneRequest(request));
      if (rejectNext) {
        rejectNext = false;
        return err(new OriginSdkError({
          source: "fake-signer",
          domain: "approval",
          code: "signing_rejected",
          message: "Fake signer rejected the request",
        }));
      }
      if (!accounts.some(({ address }) => address === request.account)) {
        return err(new OriginSdkError({
          source: "fake-signer",
          domain: "account",
          code: "account_unavailable",
          message: "Requested fake signer account is unavailable",
        }));
      }
      return ok(signature.slice());
    },
    setAccounts(value) { accounts = value.map((account) => ({ ...account })); },
    rejectNextSignature() { rejectNext = true; },
    reset() {
      accounts = initialAccounts.map((account) => ({ ...account }));
      requests.length = 0;
      rejectNext = false;
    },
  };
}
