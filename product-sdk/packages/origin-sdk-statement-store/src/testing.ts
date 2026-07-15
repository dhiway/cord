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


import type { ProductIdentity } from "@cord-network/origin-sdk-host";
import type {
  StatementDraft,
  StatementHash,
  StatementQuery,
  StatementRecord,
  StatementStoreTransport,
} from "./index.ts";
import { statementHash } from "./index.ts";

export interface FakeStatementCall {
  readonly operation: "submit" | "query" | "subscribe";
  readonly product: ProductIdentity;
  readonly value: StatementDraft | StatementQuery;
}

export interface FakeStatementTransport extends StatementStoreTransport {
  readonly calls: readonly FakeStatementCall[];
  readonly records: readonly StatementRecord[];
  activeSubscriptions(): number;
  inject(record: StatementRecord): void;
  reset(): void;
}

const cloneProduct = (product: ProductIdentity): ProductIdentity => ({ ...product });
const cloneDraft = (draft: StatementDraft): StatementDraft => ({
  ...draft, topics: [...draft.topics], data: draft.data.slice(),
});
const cloneQuery = (query: StatementQuery): StatementQuery => ({ ...query, topics: [...query.topics] });
const cloneRecord = (record: StatementRecord): StatementRecord => ({
  ...record, topics: [...record.topics], data: record.data.slice(),
});
const matches = (record: StatementRecord, query: StatementQuery): boolean =>
  query.topics.every((topic) => record.topics.includes(topic))
  && (query.kind === "broadcasts" || record.destination === query.destination);

export function createFakeStatementTransport(
  seed: readonly StatementRecord[] = [],
): FakeStatementTransport {
  const records = seed.map(cloneRecord);
  const calls: FakeStatementCall[] = [];
  const listeners = new Set<() => void>();
  let sequence = records.length;
  const notify = (): void => { for (const listener of listeners) listener(); };
  const queryRecords = (query: StatementQuery): readonly StatementRecord[] =>
    records.filter((record) => matches(record, query)).map(cloneRecord);

  return {
    get calls() {
      return calls.map((call) => ({
        ...call,
        product: cloneProduct(call.product),
        value: "data" in call.value ? cloneDraft(call.value) : cloneQuery(call.value),
      }));
    },
    get records() { return records.map(cloneRecord); },
    async submit(product, draft) {
      calls.push({ operation: "submit", product: cloneProduct(product), value: cloneDraft(draft) });
      sequence += 1;
      const hash = statementHash(`0x${sequence.toString(16).padStart(64, "0")}`);
      records.push({ ...cloneDraft(draft), hash });
      notify();
      return hash;
    },
    async query(product, query) {
      calls.push({ operation: "query", product: cloneProduct(product), value: cloneQuery(query) });
      return queryRecords(query);
    },
    async *subscribe(product, query, signal) {
      calls.push({ operation: "subscribe", product: cloneProduct(product), value: cloneQuery(query) });
      let wake: (() => void) | undefined;
      const listener = (): void => wake?.();
      listeners.add(listener);
      const abort = (): void => wake?.();
      signal.addEventListener("abort", abort);
      try {
        yield queryRecords(query);
        while (!signal.aborted) {
          await new Promise<void>((resolve) => { wake = resolve; });
          wake = undefined;
          if (!signal.aborted) yield queryRecords(query);
        }
      } finally {
        signal.removeEventListener("abort", abort);
        listeners.delete(listener);
      }
    },
    activeSubscriptions() { return listeners.size; },
    inject(record) { records.push(cloneRecord(record)); notify(); },
    reset() { records.length = 0; calls.length = 0; sequence = 0; notify(); },
  };
}
