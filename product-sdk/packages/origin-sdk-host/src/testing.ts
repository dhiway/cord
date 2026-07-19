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


import { createHash } from "node:crypto";
import { blake2b256 } from "@cord-network/origin-sdk-crypto";
import { OriginSdkError, type SdkResult } from "@cord-network/origin-sdk-errors";
import { err, ok } from "@cord-network/origin-sdk-result";
import type {
  HostAccount,
  HostCapability,
  HostFinalizedBlock,
  HostMethod,
  HostMethodMap,
  HostPreimageReference,
  HostRuntimeIdentity,
  HostStatementQuery,
  HostStatementRecord,
  HostSubscriptionMap,
  HostSubscriptionMethod,
  OriginHostBridge,
  ProductIdentity,
} from "./protocol.ts";

export interface FakeHostOptions {
  readonly accounts?: readonly HostAccount[];
  readonly now?: () => number;
  readonly finalizedBlock?: HostFinalizedBlock;
  readonly runtimeIdentity?: HostRuntimeIdentity;
}

export interface FakeHost {
  readonly bridge: OriginHostBridge;
  grant(productId: string, capability: HostCapability, expiresAt?: number): void;
  revoke(productId: string, capability: HostCapability): void;
  lose(): void;
  restore(): void;
  rejectNextSignature(): void;
  activeSubscriptions(): number;
}

const fakeHash = (byte: string): `0x${string}` => `0x${byte.repeat(64)}`;
const key = (productId: string, capability: HostCapability): string =>
  `${productId}:${capability}`;
const cloneRecord = (record: HostStatementRecord): HostStatementRecord => ({
  ...record,
  topics: [...record.topics],
  data: record.data.slice(),
});

function fakeError(domain: string, code: string, message: string): SdkResult<never> {
  return err(new OriginSdkError({ source: "fake-host", domain, code, message }));
}

function matches(record: HostStatementRecord, query: HostStatementQuery): boolean {
  if (!query.topics.every((topic) => record.topics.includes(topic))) return false;
  return query.kind === "broadcasts" || record.destination === query.destination;
}

export function createFakeHost(options: FakeHostOptions = {}): FakeHost {
  const now = options.now ?? Date.now;
  const grants = new Map<string, number | undefined>();
  const revoked = new Set<string>();
  const storage = new Map<string, Uint8Array>();
  const preimages = new Map<string, Uint8Array>();
  const statements: HostStatementRecord[] = [];
  let available = true;
  let rejectSignature = false;
  let subscriptions = 0;
  const accounts = options.accounts?.map((account) => ({ ...account })) ?? [];
  const finalizedBlock = options.finalizedBlock ?? { hash: fakeHash("1"), number: 1n };
  const runtimeIdentity = options.runtimeIdentity ?? {
    genesis_hash: fakeHash("2"),
    spec_version: 1,
    transaction_version: 1,
    metadata_hash: fakeHash("3"),
    descriptor_contract_sha256: "4".repeat(64),
    chain_spec_source_sha256: "5".repeat(64),
  };

  const ensureAvailable = (): void => {
    if (!available) throw new Error("Fake host unavailable");
  };

  const bridge: OriginHostBridge = {
    async request<Method extends HostMethod>(
      product: ProductIdentity,
      method: Method,
      input: HostMethodMap[Method]["input"],
    ): Promise<SdkResult<HostMethodMap[Method]["output"]>> {
      ensureAvailable();
      const answer = <T>(value: T): SdkResult<HostMethodMap[Method]["output"]> =>
        ok(value) as SdkResult<HostMethodMap[Method]["output"]>;
      const reject = (domain: string, code: string, message: string) =>
        fakeError(domain, code, message) as SdkResult<HostMethodMap[Method]["output"]>;
      if (method !== "permissions.authorize") {
        const capability = method.slice(0, method.indexOf(".")) as HostCapability;
        const permissionKey = key(product.id, capability);
        if (revoked.has(permissionKey)) {
          return reject("permission", "permission_revoked", "Permission revoked");
        }
        if (!grants.has(permissionKey)) {
          return reject("permission", "permission_denied", "Permission denied");
        }
        const expiresAt = grants.get(permissionKey);
        if (expiresAt !== undefined && expiresAt <= now()) {
          return reject("permission", "permission_expired", "Permission expired");
        }
      }
      switch (method) {
        case "permissions.authorize": {
          const capability = (input as HostMethodMap["permissions.authorize"]["input"]).capability;
          const permissionKey = key(product.id, capability);
          if (revoked.has(permissionKey)) {
            return reject("permission", "permission_revoked", "Permission revoked");
          }
          if (!grants.has(permissionKey)) {
            return reject("permission", "permission_denied", "Permission denied");
          }
          return answer({
            productId: product.id,
            capability,
            ...(grants.get(permissionKey) === undefined
              ? {}
              : { expiresAt: grants.get(permissionKey) }),
          });
        }
        case "accounts.list":
          return answer(accounts.map((account) => ({ ...account })));
        case "chain.finalized-block":
          return answer({ ...finalizedBlock });
        case "chain.runtime-identity":
          return answer({ ...runtimeIdentity });
        case "chain.disconnect":
          return answer(undefined);
        case "signing.approve": {
          if (rejectSignature) {
            rejectSignature = false;
            return reject("signing", "signing_rejected", "Signing rejected");
          }
          const request = input as HostMethodMap["signing.approve"]["input"];
          return answer(new Uint8Array(createHash("sha256").update(request.payload).digest()));
        }
        case "local-storage.get": {
          const { key: storageKey } = input as HostMethodMap["local-storage.get"]["input"];
          return answer(storage.get(`${product.id}:${storageKey}`)?.slice());
        }
        case "local-storage.set": {
          const { key: storageKey, value } = input as HostMethodMap["local-storage.set"]["input"];
          storage.set(`${product.id}:${storageKey}`, value.slice());
          return answer(undefined);
        }
        case "local-storage.delete": {
          const { key: storageKey } = input as HostMethodMap["local-storage.delete"]["input"];
          storage.delete(`${product.id}:${storageKey}`);
          return answer(undefined);
        }
        case "preimages.put": {
          const request = input as HostMethodMap["preimages.put"]["input"];
          const contentHash = `0x${Array.from(
            blake2b256(request.bytes),
            (byte) => byte.toString(16).padStart(2, "0"),
          ).join("")}` as const;
          preimages.set(contentHash, request.bytes.slice());
          const reference: HostPreimageReference = {
            contentHash,
            size: request.bytes.length,
            ...(request.contentType === undefined ? {} : { contentType: request.contentType }),
          };
          return answer(reference);
        }
        case "preimages.get": {
          const { contentHash } = input as HostMethodMap["preimages.get"]["input"];
          const bytes = preimages.get(contentHash);
          return bytes === undefined
            ? reject("preimages", "not_found", "Preimage not found")
            : answer(bytes.slice());
        }
        case "resources.allocate": {
          const request = input as HostMethodMap["resources.allocate"]["input"];
          return answer({
            kind: request.kind,
            account: request.account,
            authorization: new Uint8Array(createHash("sha256").update(
              `${product.id}:${request.kind}:${request.account}:${request.bytes}`,
            ).digest()),
            ...(request.expiresAt === undefined ? {} : { expiresAt: request.expiresAt }),
          });
        }
        case "statements.submit": {
          const draft = input as HostMethodMap["statements.submit"]["input"];
          const hash = `0x${createHash("sha256").update(draft.data).digest("hex")}`;
          const record: HostStatementRecord = {
            ...draft, topics: [...draft.topics], data: draft.data.slice(), hash,
          };
          statements.push(record);
          return answer(cloneRecord(record));
        }
        case "statements.query": {
          const query = input as HostMethodMap["statements.query"]["input"];
          return answer(statements.filter((record) => matches(record, query)).map(cloneRecord));
        }
        default:
          return reject("protocol", "unsupported_operation", `Unsupported host operation ${method}`);
      }
    },
    async *subscribe<Method extends HostSubscriptionMethod>(
      _product: ProductIdentity,
      method: Method,
      input: HostSubscriptionMap[Method]["input"],
      signal: AbortSignal,
    ): AsyncIterable<SdkResult<HostSubscriptionMap[Method]["output"]>> {
      ensureAvailable();
      const permissionKey = key(_product.id, "statements");
      if (revoked.has(permissionKey) || !grants.has(permissionKey)) {
        yield fakeError("permission", revoked.has(permissionKey) ? "permission_revoked" : "permission_denied", "Statement permission unavailable");
        return;
      }
      const expiresAt = grants.get(permissionKey);
      if (expiresAt !== undefined && expiresAt <= now()) {
        yield fakeError("permission", "permission_expired", "Statement permission expired");
        return;
      }
      if (method !== "statements.subscribe") {
        yield fakeError("protocol", "unsupported_operation", `Unsupported subscription ${method}`);
        return;
      }
      subscriptions += 1;
      try {
        const query = input as HostStatementQuery;
        yield ok(statements.filter((record) => matches(record, query)).map(cloneRecord)) as
          SdkResult<HostSubscriptionMap[Method]["output"]>;
        if (!signal.aborted) {
          await new Promise<void>((resolve) => signal.addEventListener("abort", () => resolve(), { once: true }));
        }
      } finally {
        subscriptions -= 1;
      }
    },
  };

  return {
    bridge,
    grant(productId, capability, expiresAt) {
      grants.set(key(productId, capability), expiresAt);
      revoked.delete(key(productId, capability));
    },
    revoke(productId, capability) {
      grants.delete(key(productId, capability));
      revoked.add(key(productId, capability));
    },
    lose() { available = false; },
    restore() { available = true; },
    rejectNextSignature() { rejectSignature = true; },
    activeSubscriptions() { return subscriptions; },
  };
}
