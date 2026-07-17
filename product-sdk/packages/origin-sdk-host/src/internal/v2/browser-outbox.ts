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
  decodeCanonicalHostV2Value,
  decodeHostV2,
  encodeHostV2,
  encodeHostV2Value,
  type HostV2Map,
} from "./codec.ts";
import type { HostOutboxEntryV1 } from "./generated.ts";

const MAX_RECORDS = 4_096;
const MAX_TOTAL_BYTES = 268_435_456;
const MAX_RECORD_BYTES = 4_456_448;
const RECOVERY_BLOCKS = 256n;

export interface BrowserOutboxEncryptedRow {
  readonly id: string;
  readonly keyVersion: number;
  readonly ciphertext: Uint8Array;
}

export interface StrictBrowserOutboxBackend {
  load(): Promise<readonly BrowserOutboxEncryptedRow[]>;
  putStrict(row: BrowserOutboxEncryptedRow): Promise<void>;
  deleteStrict(id: string): Promise<void>;
  quarantineStrict(row: BrowserOutboxEncryptedRow): Promise<void>;
}

export interface BrowserOutboxCrypto {
  seal(
    id: Uint8Array,
    plaintext: Uint8Array,
  ): Promise<{ readonly keyVersion: number; readonly ciphertext: Uint8Array }>;
  open(id: Uint8Array, keyVersion: number, ciphertext: Uint8Array): Promise<Uint8Array>;
  digest(bytes: Uint8Array): Promise<Uint8Array>;
}

export class BrowserOutboxError extends Error {
  readonly code:
    | "HOST_OUTBOX_UNAVAILABLE"
    | "HOST_OUTBOX_FULL"
    | "HOST_OUTBOX_CORRUPT"
    | "HOST_OUTBOX_EXPIRED"
    | "HOST_OUTBOX_STATE_INVALID";

  constructor(code: BrowserOutboxError["code"], message: string) {
    super(message);
    this.name = "BrowserOutboxError";
    this.code = code;
  }
}

export interface BrowserPrepareOutboxV1 {
  readonly entry: HostOutboxEntryV1;
}

export interface BrowserOutboxRetryV1 {
  readonly request: Uint8Array;
  readonly authority: Uint8Array;
  readonly requestId: Uint8Array;
  readonly outboxId: Uint8Array;
}

interface DurableRecord {
  readonly entry: HostOutboxEntryV1;
  readonly state: 0 | 1 | 2 | 3 | 4;
  readonly response?: Uint8Array;
  readonly responseAck?: Uint8Array;
  readonly responseHash?: Uint8Array;
  readonly terminal: boolean;
  readonly recoverUntil: bigint;
}

interface LoadedRecord {
  readonly record: DurableRecord;
  readonly encryptedBytes: number;
}

export class StrictIndexedDbOutboxBackend implements StrictBrowserOutboxBackend {
  readonly #database: IDBDatabase;
  readonly #records: string;
  readonly #quarantine: string;

  constructor(database: IDBDatabase, records = "host-outbox-v1", quarantine = "host-outbox-quarantine-v1") {
    this.#database = database;
    this.#records = records;
    this.#quarantine = quarantine;
  }

  async load(): Promise<readonly BrowserOutboxEncryptedRow[]> {
    const transaction = this.#database.transaction(this.#records, "readonly");
    const committed = transactionComplete(transaction);
    const request = transaction.objectStore(this.#records).getAll();
    const rows = await requestResult<BrowserOutboxEncryptedRow[]>(request);
    await committed;
    return rows.map(copyRow);
  }

  async putStrict(row: BrowserOutboxEncryptedRow): Promise<void> {
    const transaction = this.#database.transaction(this.#records, "readwrite", { durability: "strict" });
    transaction.objectStore(this.#records).put(copyRow(row));
    await transactionComplete(transaction);
  }

  async deleteStrict(id: string): Promise<void> {
    const transaction = this.#database.transaction(this.#records, "readwrite", { durability: "strict" });
    transaction.objectStore(this.#records).delete(id);
    await transactionComplete(transaction);
  }

  async quarantineStrict(row: BrowserOutboxEncryptedRow): Promise<void> {
    const transaction = this.#database.transaction(
      [this.#records, this.#quarantine],
      "readwrite",
      { durability: "strict" },
    );
    transaction.objectStore(this.#quarantine).put(copyRow(row));
    transaction.objectStore(this.#records).delete(row.id);
    await transactionComplete(transaction);
  }
}

function requestResult<T>(request: IDBRequest<T>): Promise<T> {
  return new Promise<T>((resolve, reject) => {
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error ?? new Error("IndexedDB request failed"));
  });
}

function transactionComplete(transaction: IDBTransaction): Promise<void> {
  return new Promise<void>((resolve, reject) => {
    transaction.oncomplete = () => resolve();
    transaction.onabort = () => reject(transaction.error ?? new Error("strict IndexedDB transaction aborted"));
    transaction.onerror = () => reject(transaction.error ?? new Error("strict IndexedDB transaction failed"));
  });
}

function copyRow(row: BrowserOutboxEncryptedRow): BrowserOutboxEncryptedRow {
  return { id: row.id, keyVersion: row.keyVersion, ciphertext: row.ciphertext.slice() };
}

export class BrowserHostOutboxV1 {
  readonly #backend: StrictBrowserOutboxBackend;
  readonly #crypto: BrowserOutboxCrypto;
  readonly #records = new Map<string, LoadedRecord>();
  readonly #recordLimit: number;
  readonly #byteLimit: number;

  private constructor(
    backend: StrictBrowserOutboxBackend,
    crypto: BrowserOutboxCrypto,
    recordLimit: number,
    byteLimit: number,
  ) {
    this.#backend = backend;
    this.#crypto = crypto;
    this.#recordLimit = recordLimit;
    this.#byteLimit = byteLimit;
  }

  static async open(
    backend: StrictBrowserOutboxBackend,
    crypto: BrowserOutboxCrypto,
    limits: { readonly records?: number; readonly bytes?: number } = {},
  ): Promise<BrowserHostOutboxV1> {
    const recordLimit = limits.records ?? MAX_RECORDS;
    const byteLimit = limits.bytes ?? MAX_TOTAL_BYTES;
    if (!Number.isInteger(recordLimit) || recordLimit < 1 || recordLimit > MAX_RECORDS
      || !Number.isInteger(byteLimit) || byteLimit < 1 || byteLimit > MAX_TOTAL_BYTES) {
      throw new BrowserOutboxError("HOST_OUTBOX_UNAVAILABLE", "browser outbox limits are invalid");
    }
    const outbox = new BrowserHostOutboxV1(backend, crypto, recordLimit, byteLimit);
    let rows: readonly BrowserOutboxEncryptedRow[];
    try {
      rows = await backend.load();
    } catch {
      throw new BrowserOutboxError("HOST_OUTBOX_UNAVAILABLE", "strict IndexedDB is unavailable");
    }
    if (rows.length > recordLimit) throw new BrowserOutboxError("HOST_OUTBOX_CORRUPT", "record bound exceeded");
    for (const row of rows) {
      try {
        if (!/^[0-9a-f]{32}$/.test(row.id) || row.ciphertext.length > MAX_RECORD_BYTES) throw new Error();
        const id = fromHex(row.id);
        const plaintext = await crypto.open(id, row.keyVersion, row.ciphertext.slice());
        const record = decodeRecord(plaintext);
        if (toHex(record.entry[1]) !== row.id || outbox.#records.has(row.id)) throw new Error();
        outbox.#records.set(row.id, { record, encryptedBytes: row.ciphertext.length });
      } catch {
        try {
          await backend.quarantineStrict(row);
        } catch {
          throw new BrowserOutboxError("HOST_OUTBOX_UNAVAILABLE", "corrupt browser outbox quarantine failed");
        }
        throw new BrowserOutboxError("HOST_OUTBOX_CORRUPT", "authenticated browser outbox record is corrupt");
      }
    }
    if (outbox.#totalBytes() > byteLimit) {
      throw new BrowserOutboxError("HOST_OUTBOX_CORRUPT", "encrypted browser outbox byte bound exceeded");
    }
    return outbox;
  }

  async prepare(input: BrowserPrepareOutboxV1): Promise<BrowserOutboxRetryV1> {
    const entryBytes = encodeHostV2("HostOutboxEntryV1", input.entry);
    const entry = decodeHostV2("HostOutboxEntryV1", entryBytes).value;
    if (entry[2] !== 0 || this.#records.has(toHex(entry[1]))) {
      throw new BrowserOutboxError("HOST_OUTBOX_STATE_INVALID", "browser outbox entry is not new Prepared");
    }
    decodeHostV2("RequestV2", entry[3]);
    if (!canonicalAuthority(entry[4])) {
      throw new BrowserOutboxError("HOST_OUTBOX_CORRUPT", "browser outbox authority is not canonical");
    }
    const record: DurableRecord = {
      entry,
      state: 0,
      terminal: false,
      recoverUntil: BigInt(entry[19]),
    };
    await this.#commit(record, true);
    return retry(record);
  }

  retry(outboxId: Uint8Array, finalized: bigint): BrowserOutboxRetryV1 {
    const record = this.#get(outboxId).record;
    if (finalized >= record.recoverUntil) {
      throw new BrowserOutboxError("HOST_OUTBOX_EXPIRED", "browser outbox recovery window closed");
    }
    if (record.state !== 0 && record.state !== 1) {
      throw new BrowserOutboxError("HOST_OUTBOX_STATE_INVALID", "browser outbox request is not retryable");
    }
    return retry(record);
  }

  async markSent(outboxId: Uint8Array): Promise<void> {
    const record = this.#get(outboxId).record;
    if (record.state !== 0) throw new BrowserOutboxError("HOST_OUTBOX_STATE_INVALID", "request is not Prepared");
    await this.#commit({ ...record, state: 1 }, false);
  }

  async installTerminal(
    outboxId: Uint8Array,
    response: Uint8Array,
    terminalBlock: bigint,
  ): Promise<{ readonly responseHash: Uint8Array; readonly ack: Uint8Array }> {
    const loaded = this.#get(outboxId);
    const record = loaded.record;
    const event = decodeHostV2("EventV2", response).value;
    if (record.state === 2) {
      if (!equal(record.response!, response)) {
        throw new BrowserOutboxError("HOST_OUTBOX_STATE_INVALID", "terminal response changed");
      }
      return { responseHash: record.responseHash!.slice(), ack: record.responseAck!.slice() };
    }
    if ((record.state !== 0 && record.state !== 1) || ![2, 3, 4].includes(event[3])) {
      throw new BrowserOutboxError("HOST_OUTBOX_STATE_INVALID", "response is not terminal");
    }
    const responseHash = await this.#crypto.digest(response);
    if (responseHash.length !== 32) {
      throw new BrowserOutboxError("HOST_OUTBOX_CORRUPT", "browser digest provider returned wrong length");
    }
    const ack = encodeHostV2("ResponseAckV1", {
      0: record.entry[6],
      1: record.entry[7],
      2: record.entry[8],
      3: responseHash,
    });
    const installed: DurableRecord = {
      ...record,
      state: 2,
      response: response.slice(),
      responseAck: ack,
      responseHash: responseHash.slice(),
      terminal: true,
      recoverUntil: terminalBlock + RECOVERY_BLOCKS,
    };
    await this.#commit(installed, false);
    return { responseHash: responseHash.slice(), ack: ack.slice() };
  }

  installedAck(outboxId: Uint8Array): { readonly responseHash: Uint8Array; readonly ack: Uint8Array } {
    const record = this.#get(outboxId).record;
    if (record.state !== 2 && record.state !== 3) {
      throw new BrowserOutboxError("HOST_OUTBOX_STATE_INVALID", "response acknowledgement is absent");
    }
    return { responseHash: record.responseHash!.slice(), ack: record.responseAck!.slice() };
  }

  async confirmAck(outboxId: Uint8Array, responseHash: Uint8Array): Promise<void> {
    const record = this.#get(outboxId).record;
    if (record.state === 3 && equal(record.responseHash!, responseHash)) return;
    if (record.state !== 2 || !equal(record.responseHash!, responseHash)) {
      throw new BrowserOutboxError("HOST_OUTBOX_STATE_INVALID", "response acknowledgement mismatches");
    }
    await this.#commit({ ...record, state: 3 }, false);
  }

  async expire(outboxId: Uint8Array, finalized: bigint): Promise<void> {
    const record = this.#get(outboxId).record;
    if (finalized < record.recoverUntil || record.state > 1) {
      throw new BrowserOutboxError("HOST_OUTBOX_STATE_INVALID", "browser outbox cannot expire yet");
    }
    await this.#commit({ ...record, state: 4 }, false);
  }

  async gc(finalized: bigint, limit: number): Promise<number> {
    const eligible = [...this.#records.entries()]
      .filter(([, loaded]) => loaded.record.terminal
        && loaded.record.state === 3
        && finalized >= loaded.record.recoverUntil)
      .slice(0, Math.max(0, limit));
    for (const [id] of eligible) {
      try {
        await this.#backend.deleteStrict(id);
      } catch {
        throw new BrowserOutboxError("HOST_OUTBOX_UNAVAILABLE", "strict IndexedDB GC commit failed");
      }
      this.#records.delete(id);
    }
    return eligible.length;
  }

  async #commit(record: DurableRecord, create: boolean): Promise<void> {
    const idBytes = record.entry[1];
    const id = toHex(idBytes);
    const plaintext = encodeRecord(record);
    let sealed: { readonly keyVersion: number; readonly ciphertext: Uint8Array };
    try {
      sealed = await this.#crypto.seal(idBytes.slice(), plaintext);
    } catch {
      throw new BrowserOutboxError("HOST_OUTBOX_UNAVAILABLE", "browser outbox key or crypto is unavailable");
    }
    if (!Number.isInteger(sealed.keyVersion) || sealed.keyVersion < 0
      || sealed.ciphertext.length > MAX_RECORD_BYTES) {
      throw new BrowserOutboxError("HOST_OUTBOX_FULL", "encrypted browser outbox record is too large");
    }
    const previous = this.#records.get(id);
    if ((create && (previous || this.#records.size >= this.#recordLimit))
      || this.#totalBytes() - (previous?.encryptedBytes ?? 0) + sealed.ciphertext.length > this.#byteLimit) {
      throw new BrowserOutboxError("HOST_OUTBOX_FULL", "browser outbox capacity is full");
    }
    const row = { id, keyVersion: sealed.keyVersion, ciphertext: sealed.ciphertext.slice() };
    try {
      await this.#backend.putStrict(row);
    } catch {
      throw new BrowserOutboxError("HOST_OUTBOX_UNAVAILABLE", "strict IndexedDB commit failed");
    }
    this.#records.set(id, { record, encryptedBytes: row.ciphertext.length });
  }

  #get(outboxId: Uint8Array): LoadedRecord {
    const loaded = this.#records.get(toHex(outboxId));
    if (!loaded) throw new BrowserOutboxError("HOST_OUTBOX_STATE_INVALID", "browser outbox entry is absent");
    return loaded;
  }

  #totalBytes(): number {
    let total = 0;
    for (const loaded of this.#records.values()) total += loaded.encryptedBytes;
    return total;
  }
}

function retry(record: DurableRecord): BrowserOutboxRetryV1 {
  return {
    request: record.entry[3].slice(),
    authority: record.entry[4].slice(),
    requestId: record.entry[6].slice(),
    outboxId: record.entry[1].slice(),
  };
}

function canonicalAuthority(bytes: Uint8Array): boolean {
  try {
    decodeHostV2("ProviderCapabilityV1", bytes);
    return true;
  } catch {
    try {
      decodeHostV2("ResumeTokenV1", bytes);
      return true;
    } catch {
      return false;
    }
  }
}

function encodeRecord(record: DurableRecord): Uint8Array {
  const map: HostV2Map = {
    0: 1,
    1: encodeHostV2("HostOutboxEntryV1", record.entry),
    2: record.state,
    6: record.terminal,
    7: record.recoverUntil,
    ...(record.response ? { 3: record.response } : {}),
    ...(record.responseAck ? { 4: record.responseAck } : {}),
    ...(record.responseHash ? { 5: record.responseHash } : {}),
  };
  return encodeHostV2Value(map);
}

function decodeRecord(bytes: Uint8Array): DurableRecord {
  const value = decodeCanonicalHostV2Value(bytes);
  if (typeof value !== "object" || value === null || Array.isArray(value) || value instanceof Uint8Array) {
    throw new Error("record map required");
  }
  const map = value as HostV2Map;
  const keys = Object.keys(map).map(Number).sort((left, right) => left - right);
  if (keys.some((key) => ![0, 1, 2, 3, 4, 5, 6, 7].includes(key))
    || ![0, 1, 2, 6, 7].every((key) => keys.includes(key))
    || map[0] !== 1
    || !(map[1] instanceof Uint8Array)
    || typeof map[2] !== "number"
    || ![0, 1, 2, 3, 4].includes(map[2])
    || typeof map[6] !== "boolean"
    || (typeof map[7] !== "number" && typeof map[7] !== "bigint")) {
    throw new Error("closed record invalid");
  }
  const entry = decodeHostV2("HostOutboxEntryV1", map[1]).value;
  const state = map[2] as DurableRecord["state"];
  const response = optionalBytes(map[3]);
  const responseAck = optionalBytes(map[4]);
  const responseHash = optionalBytes(map[5], 32);
  const hasInstalledResponse = Boolean(response && responseAck && responseHash);
  if ((state === 2 || state === 3) !== hasInstalledResponse
    || ((state === 2 || state === 3) !== map[6])) {
    throw new Error("record lifecycle invalid");
  }
  return {
    entry,
    state,
    ...(response ? { response } : {}),
    ...(responseAck ? { responseAck } : {}),
    ...(responseHash ? { responseHash } : {}),
    terminal: map[6],
    recoverUntil: BigInt(map[7] as number | bigint),
  };
}

function optionalBytes(value: unknown, length?: number): Uint8Array | undefined {
  if (value === undefined) return undefined;
  if (!(value instanceof Uint8Array) || (length !== undefined && value.length !== length)) throw new Error();
  return value.slice();
}

function equal(left: Uint8Array, right: Uint8Array): boolean {
  return left.length === right.length && left.every((byte, index) => byte === right[index]);
}

function toHex(bytes: Uint8Array): string {
  return [...bytes].map((byte) => byte.toString(16).padStart(2, "0")).join("");
}

function fromHex(value: string): Uint8Array {
  return Uint8Array.from(value.match(/../g)!.map((byte) => Number.parseInt(byte, 16)));
}
