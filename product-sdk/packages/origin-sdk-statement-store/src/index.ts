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
import type { HostStatementRecord, OriginHostClient, ProductIdentity } from "@cord-network/origin-sdk-host";
import { hash32, type AccountId, type Hash32 } from "@cord-network/origin-sdk-identity";
import type { ResourcesClient, StatementAllowance } from "@cord-network/origin-sdk-resources";
import { err, ok } from "@cord-network/origin-sdk-result";

declare const statementType: unique symbol;
export type StatementHash = Hash32 & { readonly [statementType]: "StatementHash" };
export type StatementTopic = Hash32 & { readonly [statementType]: "StatementTopic" };
export type StatementDestination = Hash32 & { readonly [statementType]: "StatementDestination" };
export type StatementChannel = Hash32 & { readonly [statementType]: "StatementChannel" };

export interface StatementDraft {
  readonly account: AccountId;
  readonly topics: readonly StatementTopic[];
  readonly data: Uint8Array;
  readonly destination?: StatementDestination;
  readonly channel?: StatementChannel;
  readonly priority?: number;
}

export interface StatementRecord {
  readonly hash: StatementHash;
  readonly account: AccountId;
  readonly topics: readonly StatementTopic[];
  readonly data: Uint8Array;
  readonly destination?: StatementDestination;
  readonly channel?: StatementChannel;
  readonly priority?: number;
}

export type StatementQuery =
  | { readonly kind: "broadcasts"; readonly topics: readonly StatementTopic[] }
  | { readonly kind: "posted"; readonly topics: readonly StatementTopic[]; readonly destination: StatementDestination }
  | { readonly kind: "posted-clear"; readonly topics: readonly StatementTopic[]; readonly destination: StatementDestination };

export interface StatementStoreTransport {
  /** Host signs, encodes, and submits through `statement_submit`. */
  submit(product: ProductIdentity, draft: StatementDraft, signal?: AbortSignal): Promise<StatementHash>;
  /** Host queries the matching safe statement RPC and returns decoded records. */
  query(product: ProductIdentity, query: StatementQuery, signal?: AbortSignal): Promise<readonly StatementRecord[]>;
  /** Reconnecting, disposable host stream; completion is terminal. */
  subscribe(product: ProductIdentity, query: StatementQuery, signal: AbortSignal): AsyncIterable<readonly StatementRecord[]>;
}

export interface StatementStoreClient {
  allowances(account: AccountId, signal?: AbortSignal): Promise<SdkResult<readonly StatementAllowance[]>>;
  submit(draft: StatementDraft, signal?: AbortSignal): Promise<SdkResult<StatementHash>>;
  query(query: StatementQuery, signal?: AbortSignal): Promise<SdkResult<readonly StatementRecord[]>>;
  subscribe(query: StatementQuery, signal?: AbortSignal): AsyncIterable<SdkResult<readonly StatementRecord[]>>;
}

export const STATEMENT_STORE_BINDINGS = {
  nodeFlag: "--enable-statement-store",
  submit: "statement_submit",
  queries: ["statement_broadcastsStatement", "statement_postedStatement", "statement_postedClearStatement"],
  allowance: "Resources.set_statement_store_account",
  finality: "node-store-after-runtime-validation",
} as const;

export const STATEMENT_STORE_EXCLUSIONS = [
  { target: "statement_dump", reason: "unsafe whole-store administration" },
  { target: "statement_remove", reason: "node-local destructive administration" },
] as const;

const invalid = <T>(message: string): SdkResult<T> => err(new OriginSdkError({
  source: "statement-store", domain: "input", code: "invalid_input", message, retryable: false,
}));
const abortError = () => new OriginSdkError({
  source: "statement-store", domain: "subscription", code: "cancelled", message: "Statement subscription cancelled", retryable: false,
});
const validateTopics = (topics: readonly StatementTopic[]): void => {
  if (topics.length > 4 || new Set(topics).size !== topics.length) throw new TypeError("topics must contain at most four unique hashes");
  for (const topic of topics) hash32(topic);
};
const validateQuery = (query: StatementQuery): void => {
  validateTopics(query.topics);
  if (query.kind !== "broadcasts") hash32(query.destination);
};
const validateDraft = (draft: StatementDraft): void => {
  if (!draft.account || draft.account.length > 128) throw new TypeError("account must contain 1-128 characters");
  validateTopics(draft.topics);
  if (!(draft.data instanceof Uint8Array) || draft.data.length < 1 || draft.data.length > 1024 * 1024) throw new TypeError("statement data must contain 1-1048576 bytes");
  if (draft.destination !== undefined) hash32(draft.destination);
  if (draft.channel !== undefined) hash32(draft.channel);
  if (draft.priority !== undefined && (!Number.isSafeInteger(draft.priority) || draft.priority < 0 || draft.priority > 0xffff_ffff)) throw new TypeError("priority must be a u32");
};
const cloneDraft = (draft: StatementDraft): StatementDraft => ({ ...draft, topics: [...draft.topics], data: draft.data.slice() });
const cloneQuery = (query: StatementQuery): StatementQuery => ({ ...query, topics: [...query.topics] });
const cloneRecords = (records: readonly StatementRecord[]): readonly StatementRecord[] => records.map((record) => ({ ...record, topics: [...record.topics], data: record.data.slice() }));

export const statementTopic = (value: string): StatementTopic => { hash32(value); return value as StatementTopic; };
export const statementDestination = (value: string): StatementDestination => { hash32(value); return value as StatementDestination; };
export const statementChannel = (value: string): StatementChannel => { hash32(value); return value as StatementChannel; };
export const statementHash = (value: string): StatementHash => { hash32(value); return value as StatementHash; };

export function createStatementStoreClient(
  host: OriginHostClient,
  transport: StatementStoreTransport,
  resources: ResourcesClient,
): StatementStoreClient {
  const permission = (signal?: AbortSignal) => host.authorize("statements", signal);
  return {
    allowances: (account, signal) => resources.statementAllowances(account, signal),
    async submit(draft, signal) {
      try { validateDraft(draft); } catch (error) { return invalid(error instanceof Error ? error.message : "invalid statement"); }
      const grant = await permission(signal); if (!grant.success) return grant;
      try { return ok(await transport.submit(host.product, cloneDraft(draft), signal)); }
      catch (error) { return err(asSdkError(error, { source: "statement-store", domain: "submit", code: signal?.aborted ? "cancelled" : "submit_failed", retryable: !signal?.aborted })); }
    },
    async query(query, signal) {
      try { validateQuery(query); } catch (error) { return invalid(error instanceof Error ? error.message : "invalid query"); }
      const grant = await permission(signal); if (!grant.success) return grant;
      try { return ok(cloneRecords(await transport.query(host.product, cloneQuery(query), signal))); }
      catch (error) { return err(asSdkError(error, { source: "statement-store", domain: "query", code: signal?.aborted ? "cancelled" : "query_failed", retryable: !signal?.aborted })); }
    },
    async *subscribe(query, signal = new AbortController().signal) {
      try { validateQuery(query); } catch (error) { yield invalid(error instanceof Error ? error.message : "invalid query"); return; }
      const grant = await permission(signal); if (!grant.success) { yield grant; return; }
      try {
        for await (const records of transport.subscribe(host.product, cloneQuery(query), signal)) {
          if (signal.aborted) { yield err(abortError()); return; }
          yield ok(cloneRecords(records));
        }
      } catch (error) {
        yield err(signal.aborted ? abortError() : asSdkError(error, { source: "statement-store", domain: "subscription", code: "subscription_failed", retryable: true }));
      }
    },
  };
}

function requireHostStatementValue<T>(result: SdkResult<T>): T {
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

function statementFromHost(record: HostStatementRecord): StatementRecord {
  return {
    hash: statementHash(record.hash),
    account: record.account as AccountId,
    topics: record.topics.map(statementTopic),
    data: record.data.slice(),
    ...(record.destination === undefined
      ? {}
      : { destination: statementDestination(record.destination) }),
    ...(record.channel === undefined ? {} : { channel: statementChannel(record.channel) }),
    ...(record.priority === undefined ? {} : { priority: record.priority }),
  };
}

/** Host-backed statement transport used by the umbrella; no direct node endpoint is accepted. */
export function createHostStatementStoreTransport(
  host: OriginHostClient,
): StatementStoreTransport {
  const assertProduct = (product: ProductIdentity): void => {
    if (product.id !== host.product.id) {
      throw new OriginSdkError({
        source: "statement-store",
        domain: "host",
        code: "product_mismatch",
        message: "Statement transport product does not match the active host client",
      });
    }
  };
  return {
    async submit(product, draft, signal) {
      assertProduct(product);
      const record = requireHostStatementValue(await host.submitStatement(draft, signal));
      return statementHash(record.hash);
    },
    async query(product, query, signal) {
      assertProduct(product);
      const records = requireHostStatementValue(await host.queryStatements(query, signal));
      return records.map(statementFromHost);
    },
    async *subscribe(product, query, signal) {
      assertProduct(product);
      for await (const result of host.subscribeStatements(query, signal)) {
        yield requireHostStatementValue(result).map(statementFromHost);
      }
    },
  };
}
