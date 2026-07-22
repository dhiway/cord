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


import { COMMONS_NETWORK_BINDING } from "@cord-network/origin-sdk-descriptors";
import { OriginSdkError } from "@cord-network/origin-sdk-errors";
import type { HostAccount, ProductIdentity } from "@cord-network/origin-sdk-host";
import { createFakeHost, type FakeHost } from "@cord-network/origin-sdk-host/testing";
import { fakeTransaction } from "@cord-network/origin-sdk-tx/testing";
import {
  createApp,
  type CommonsRuntimeExecutor,
  type OriginApp,
  type OriginAppRuntime,
} from "./index.ts";

export * from "@cord-network/origin-sdk-host/testing";
export * from "@cord-network/origin-sdk-local-storage/testing";
export * from "@cord-network/origin-sdk-signer/testing";
export * from "@cord-network/origin-sdk-statement-store/testing";
export * from "@cord-network/origin-sdk-tx/testing";

export interface FakePreparedRuntimeCall {
  readonly at: `0x${string}`;
  readonly target: string;
  readonly payload: Readonly<Record<string, unknown>>;
  readonly context?: Readonly<Record<string, unknown>>;
}

export type FakeAppOverrides = Partial<Pick<
  OriginApp,
  | "identity"
  | "personhood"
  | "resources"
  | "attestations"
  | "names"
  | "cloudStorage"
  | "statements"
  | "assets"
  | "apps"
>>;

export interface CreateFakeAppOptions {
  readonly product?: ProductIdentity;
  readonly account?: HostAccount;
  readonly runtime?: OriginAppRuntime | CommonsRuntimeExecutor;
  readonly overrides?: FakeAppOverrides;
  readonly storageNamespace?: string;
}

export interface FakeOriginApp extends OriginApp {
  readonly testing: {
    readonly host: FakeHost;
    readonly prepared: readonly FakePreparedRuntimeCall[];
    reset(): void;
  };
}

const runtimeIdentity = {
  genesis_hash: COMMONS_NETWORK_BINDING.genesis_hash,
  spec_version: COMMONS_NETWORK_BINDING.spec_version,
  transaction_version: COMMONS_NETWORK_BINDING.transaction_version,
  metadata_hash: COMMONS_NETWORK_BINDING.metadata_hash,
  descriptor_contract_sha256: COMMONS_NETWORK_BINDING.descriptor_contract_sha256,
  chain_spec_source_sha256: COMMONS_NETWORK_BINDING.chain_spec_source_sha256,
};

/**
 * Create an in-memory hosted application for application logic tests.
 * Native reads fail closed unless a domain override or runtime is supplied; this does not fake RPC.
 */
export async function createFakeApp(
  options: CreateFakeAppOptions = {},
): Promise<FakeOriginApp> {
  const product = options.product ?? { id: "fake.origin.app", name: "Fake Origin App" };
  const account = options.account ?? { address: "5FakeOrigin", name: "Fake Origin" };
  const host = createFakeHost({ accounts: [account], runtimeIdentity });
  for (const capability of [
    "accounts", "chain", "signing", "local-storage", "preimages", "resources", "statements",
  ] as const) host.grant(product.id, capability);

  const prepared: FakePreparedRuntimeCall[] = [];
  const runtime = options.runtime ?? {
    async read(_at, target) {
      throw new OriginSdkError({
        source: "fake-app",
        domain: "chain-read",
        code: "unconfigured_chain_read",
        message: `Fake app native read ${target} is not configured; override the domain or use E2E wiring`,
      });
    },
    async prepare(at, target, payload, context) {
      prepared.push({
        at,
        target,
        payload: { ...payload },
        ...(context === undefined ? {} : { context: { ...context } }),
      });
      return fakeTransaction([
        { type: "finalized", blockHash: at, transactionHash: `0x${"44".repeat(32)}` },
      ]);
    },
  } satisfies CommonsRuntimeExecutor;
  const created = await createApp({
    product,
    bridge: host.bridge,
    runtime,
    account: account.address,
    ...(options.storageNamespace === undefined ? {} : { storageNamespace: options.storageNamespace }),
  });
  if (!created.success) throw created.error;
  const app = { ...created.value, ...options.overrides } as FakeOriginApp;
  Object.defineProperty(app, "testing", {
    enumerable: true,
    value: {
      host,
      get prepared() { return prepared.map((call) => ({ ...call, payload: { ...call.payload } })); },
      reset() { prepared.length = 0; },
    },
  });
  return app;
}
