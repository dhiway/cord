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


import {
  createAssetsClient,
  type AssetsClient,
  type AssetsRuntimeAdapter,
} from "@cord-network/origin-sdk-assets";
import {
  createOriginAppDeployer,
  createOriginAppsClient,
  type PrepareOriginAppDeployment,
  type PreparedOriginAppDeployment,
  type OriginAppsClient,
  type OriginAppContentStore,
  type OriginAppBlockStore,
} from "@cord-network/origin-sdk-apps";
import {
  createAttestationClient,
  type AttestationClient,
  type AttestationRuntimeAdapter,
} from "@cord-network/origin-sdk-attestation";
import {
  createCommonsChainClient,
  type CommonsChainClient,
} from "@cord-network/origin-sdk-chain-client";
import {
  createCloudStorageClient,
  type CloudStorageClient,
  type CloudStorageRuntimeAdapter,
} from "@cord-network/origin-sdk-cloud-storage";
import { OriginSdkError, asSdkError, type SdkResult } from "@cord-network/origin-sdk-errors";
import {
  createHostChainProvider,
  createHostClient,
  type OriginHostBridge,
  type OriginHostClient,
  type ProductIdentity,
} from "@cord-network/origin-sdk-host";
import {
  createIdentityV2Client,
  createTransactionSigningV2Client,
  IdentityReplayJournalV2,
  type IdentityV2Bridge,
  type IdentityV2Client,
  type TransactionSigningV2Client,
} from "@cord-network/origin-sdk-identity";
import {
  createLocalStorage,
  type LocalStorageClient,
} from "@cord-network/origin-sdk-local-storage";
import {
  createNamesClient,
  type NamesClient,
  type NamesRuntimeAdapter,
} from "@cord-network/origin-sdk-names";
import { err, ok } from "@cord-network/origin-sdk-result";
import {
  selectHostSigner,
  type SelectedOriginSigner,
} from "@cord-network/origin-sdk-signer";
import {
  createOriginAppRuntime,
  type CommonsRuntimeExecutor,
} from "./runtime.ts";

export * from "./runtime.ts";

export interface OriginAppRuntime {
  readonly attestation: AttestationRuntimeAdapter;
  readonly names: NamesRuntimeAdapter;
  readonly storage: CloudStorageRuntimeAdapter;
  readonly assets: AssetsRuntimeAdapter;
}

export interface OriginApplicationClient extends OriginAppsClient {
  prepareDeployment(
    input: PrepareOriginAppDeployment,
    signal?: AbortSignal,
  ): Promise<SdkResult<PreparedOriginAppDeployment>>;
}

export interface CreateAppOptions {
  readonly product: ProductIdentity;
  readonly bridge: OriginHostBridge;
  /** Host-v2 transport for independently granted Identity operations. */
  readonly identityBridge: IdentityV2Bridge;
  /** One descriptor-backed integration bundle supplied by the host/platform integration. */
  readonly runtime: OriginAppRuntime | CommonsRuntimeExecutor;
  /** Authenticated native-provider adapter for application manifest bytes. */
  readonly content: OriginAppContentStore;
  /** Authenticated native-provider adapter for raw application blocks. */
  readonly blocks: OriginAppBlockStore;
  readonly account?: string;
  readonly storageNamespace?: string;
  readonly signal?: AbortSignal;
}

export interface OriginApp {
  readonly product: ProductIdentity;
  readonly host: OriginHostClient;
  readonly chain: CommonsChainClient;
  readonly signer: SelectedOriginSigner;
  readonly storage: LocalStorageClient;
  readonly identity: IdentityV2Client;
  /** Fresh-consent transaction signing is deliberately separate from Identity reads/proofs. */
  readonly signing: TransactionSigningV2Client;
  readonly attestations: AttestationClient;
  readonly names: NamesClient;
  readonly cloudStorage: CloudStorageClient;
  readonly assets: AssetsClient;
  readonly apps: OriginApplicationClient;
  readonly signal: AbortSignal;
  close(): Promise<SdkResult<void>>;
}

function appError(
  code: string,
  message: string,
  retryable = false,
): SdkResult<never> {
  return err(new OriginSdkError({
    source: "origin-sdk",
    domain: "app",
    code,
    message,
    retryable,
  }));
}

export async function createApp(
  options: CreateAppOptions,
): Promise<SdkResult<OriginApp>> {
  if (options.signal?.aborted) return appError("cancelled", "Application bootstrap cancelled");
  const lifetime = new AbortController();
  const forwardAbort = () => lifetime.abort(options.signal?.reason);
  options.signal?.addEventListener("abort", forwardAbort, { once: true });
  let host: OriginHostClient;
  try { host = createHostClient(options.bridge, options.product); }
  catch (error) {
    options.signal?.removeEventListener("abort", forwardAbort);
    return err(asSdkError(error, {
      source: "origin-sdk",
      domain: "app",
      code: "invalid_configuration",
      retryable: false,
    }));
  }
  const chain = createCommonsChainClient(createHostChainProvider(host));
  const disconnectQuietly = async (): Promise<void> => {
    try { await chain.disconnect(); } catch { /* initialization failure is already authoritative */ }
  };
  const snapshot = await chain.finalizedSnapshot(lifetime.signal);
  if (!snapshot.success) {
    await disconnectQuietly();
    options.signal?.removeEventListener("abort", forwardAbort);
    return snapshot;
  }
  const selected = await selectHostSigner(host, options.account, lifetime.signal);
  if (!selected.success) {
    await disconnectQuietly();
    options.signal?.removeEventListener("abort", forwardAbort);
    return selected;
  }

  const runtime = "read" in options.runtime
    ? createOriginAppRuntime(options.runtime)
    : options.runtime;
  const identityReplay = new IdentityReplayJournalV2();
  const identity = createIdentityV2Client(options.product.id, options.identityBridge, identityReplay);
  const signing = createTransactionSigningV2Client(
    options.product.id,
    options.identityBridge,
    identityReplay,
  );
  const cloudStorage = createCloudStorageClient(chain, runtime.storage);
  const apps = createOriginAppsClient(chain, runtime.names, options.content);
  const deployer = createOriginAppDeployer(
    apps,
    cloudStorage,
    options.blocks,
  );
  let closed = false;
  const app: OriginApp = {
    product: { ...options.product },
    host,
    chain,
    signer: selected.value,
    storage: createLocalStorage(host, options.storageNamespace ?? "app"),
    identity,
    signing,
    attestations: createAttestationClient(chain, runtime.attestation),
    names: createNamesClient(chain, runtime.names),
    cloudStorage,
    assets: createAssetsClient(chain, runtime.assets),
    apps: { ...apps, prepareDeployment: deployer.prepare },
    signal: lifetime.signal,
    async close() {
      if (closed) return ok(undefined);
      closed = true;
      lifetime.abort();
      options.signal?.removeEventListener("abort", forwardAbort);
      try {
        await chain.disconnect();
        return ok(undefined);
      } catch (error) {
        return err(asSdkError(error, {
          source: "origin-sdk",
          domain: "lifecycle",
          code: "disconnect_failed",
          retryable: true,
        }));
      }
    },
  };
  return ok(app);
}

export const ORIGIN_APP_CONTRACT = {
  network: "Commons",
  hosted: true,
  endpointSelection: "host-only",
  nativeDomains: [
    "identity", "signing", "attestations", "names", "cloudStorage", "assets",
  ],
  contractsIncluded: false,
  applicationDomains: ["apps"],
} as const;
