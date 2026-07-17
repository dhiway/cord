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

import { decodeCanonicalHostV2Value, decodeHostV2, encodeHostV2, encodeHostV2Value, type HostV2Map } from "./codec.ts";
import { HOST_V2_OPERATION_BINDINGS, type HostOutboxEntryV1 } from "./generated.ts";
import {
  BrowserHostOutboxKeyRingV1,
  BrowserXChaCha20Poly1305,
  copyContext,
  type BrowserHostOutboxContextV1,
  type BrowserRandomBytes,
} from "./browser-crypto.ts";

const MAX_RECORDS = 4_096; const MAX_TOTAL_BYTES = 268_435_456; const MAX_RECORD_BYTES = 4_456_448;
const AUTHORITY_BLOCKS = 128n; const RECOVERY_BLOCKS = 256n; const MAX_RECOVERY_BLOCKS = 384n;
const PROVIDER_BYTE_OPERATION_CODES = new Set([1010, 1011, 1012, 1014]);

export interface BrowserOutboxEncryptedRow { readonly id: string; readonly keyVersion: number; readonly ciphertext: Uint8Array }
export interface StrictBrowserOutboxTransactionV1 {
  readonly expected: Readonly<Record<string, BrowserOutboxEncryptedRow | null>>;
  readonly puts: readonly BrowserOutboxEncryptedRow[];
  readonly deletes: readonly string[];
}
export interface StrictBrowserOutboxBackend {
  load(): Promise<readonly BrowserOutboxEncryptedRow[]>;
  putStrict(row: BrowserOutboxEncryptedRow): Promise<void>;
  deleteStrict(id: string): Promise<void>;
  quarantineStrict(row: BrowserOutboxEncryptedRow): Promise<void>;
  transactStrict(transaction: StrictBrowserOutboxTransactionV1): Promise<void>;
}
export class BrowserOutboxError extends Error {
  readonly code: "HOST_OUTBOX_UNAVAILABLE" | "HOST_OUTBOX_FULL" | "HOST_OUTBOX_CORRUPT" | "HOST_OUTBOX_EXPIRED" | "HOST_OUTBOX_STATE_INVALID" | "HOST_OUTBOX_BINDING_INVALID";
  constructor(code: BrowserOutboxError["code"], message: string) { super(message); this.name = "BrowserOutboxError"; this.code = code; }
}
export interface BrowserPrepareOutboxV1 { readonly entry: HostOutboxEntryV1 }
export interface BrowserOutboxRetryV1 {
  readonly request: Uint8Array; readonly authority: Uint8Array; readonly requestId: Uint8Array;
  readonly operationId: Uint8Array; readonly outboxId: Uint8Array; readonly fingerprint: Uint8Array;
  readonly expectedResponseKind: number; readonly intendedCursor: number; readonly cancel: boolean;
  readonly operationCode: number;
}
interface LiveRecord {
  readonly kind: 0; readonly entry: HostOutboxEntryV1; readonly state: 0 | 1 | 2;
  readonly response?: Uint8Array; readonly responseAck?: Uint8Array; readonly responseHash?: Uint8Array;
  readonly terminal: boolean; readonly recoverUntil: bigint; readonly operationCode: number;
}
interface TombstoneRecord {
  readonly kind: 1; readonly outboxId: Uint8Array; readonly state: 3 | 4;
  readonly responseHash?: Uint8Array; readonly requestFingerprint: Uint8Array;
  readonly terminal: boolean; readonly recoverUntil: bigint; readonly keyVersion: number;
}
type DurableRecord = LiveRecord | TombstoneRecord;
interface LoadedRecord { readonly record: DurableRecord; readonly encryptedBytes: number }
interface ValidatedEntry { readonly operationCode: number; readonly operationIdRequired: boolean }

export class StrictIndexedDbOutboxBackend implements StrictBrowserOutboxBackend {
  readonly #database: IDBDatabase; readonly #records: string; readonly #quarantine: string;
  constructor(database: IDBDatabase, records = "host-outbox-v1", quarantine = "host-outbox-quarantine-v1") { this.#database = database; this.#records = records; this.#quarantine = quarantine; }
  async load(): Promise<readonly BrowserOutboxEncryptedRow[]> {
    const transaction = this.#database.transaction(this.#records, "readonly"); const committed = transactionComplete(transaction);
    const rows = await requestResult<BrowserOutboxEncryptedRow[]>(transaction.objectStore(this.#records).getAll()); await committed; return rows.map(copyRow);
  }
  async putStrict(row: BrowserOutboxEncryptedRow): Promise<void> {
    const transaction = this.#database.transaction(this.#records, "readwrite", { durability: "strict" }); transaction.objectStore(this.#records).put(copyRow(row)); await transactionComplete(transaction);
  }
  async deleteStrict(id: string): Promise<void> {
    const transaction = this.#database.transaction(this.#records, "readwrite", { durability: "strict" }); transaction.objectStore(this.#records).delete(id); await transactionComplete(transaction);
  }
  async quarantineStrict(row: BrowserOutboxEncryptedRow): Promise<void> {
    const transaction = this.#database.transaction([this.#records, this.#quarantine], "readwrite", { durability: "strict" });
    transaction.objectStore(this.#quarantine).put(copyRow(row)); transaction.objectStore(this.#records).delete(row.id); await transactionComplete(transaction);
  }
  async transactStrict(input: StrictBrowserOutboxTransactionV1): Promise<void> {
    const transaction = this.#database.transaction(this.#records, "readwrite", { durability: "strict" });
    const store = transaction.objectStore(this.#records);
    for (const [id, expected] of Object.entries(input.expected)) {
      const current = await requestResult<BrowserOutboxEncryptedRow | undefined>(store.get(id));
      if (!sameOptionalRow(current, expected)) { transaction.abort(); throw new Error("strict IndexedDB compare-and-swap failed"); }
    }
    for (const row of input.puts) store.put(copyRow(row));
    for (const id of input.deletes) store.delete(id);
    await transactionComplete(transaction);
  }
}
function requestResult<T>(request: IDBRequest<T>): Promise<T> { return new Promise((resolve, reject) => { request.onsuccess = () => resolve(request.result); request.onerror = () => reject(request.error ?? new Error("IndexedDB request failed")); }); }
function transactionComplete(transaction: IDBTransaction): Promise<void> { return new Promise((resolve, reject) => { transaction.oncomplete = () => resolve(); transaction.onabort = () => reject(transaction.error ?? new Error("strict IndexedDB transaction aborted")); transaction.onerror = () => reject(transaction.error ?? new Error("strict IndexedDB transaction failed")); }); }
function copyRow(row: BrowserOutboxEncryptedRow): BrowserOutboxEncryptedRow { return { id: row.id, keyVersion: row.keyVersion, ciphertext: row.ciphertext.slice() }; }
function sameOptionalRow(left: BrowserOutboxEncryptedRow | undefined, right: BrowserOutboxEncryptedRow | null): boolean {
  if (!left || !right) return left === undefined && right === null;
  return left.id === right.id && left.keyVersion === right.keyVersion && equal(left.ciphertext, right.ciphertext);
}

export class BrowserHostOutboxV1 {
  readonly #backend: StrictBrowserOutboxBackend; readonly #crypto: BrowserXChaCha20Poly1305;
  readonly #context: BrowserHostOutboxContextV1; readonly #records = new Map<string, LoadedRecord>();
  readonly #preparingOutboxIds = new Set<string>(); readonly #preparingOperationGenerations = new Set<string>();
  readonly #recordLimit: number; readonly #byteLimit: number;
  #commitTail = Promise.resolve();
  private constructor(backend: StrictBrowserOutboxBackend, crypto: BrowserXChaCha20Poly1305, context: BrowserHostOutboxContextV1, recordLimit: number, byteLimit: number) {
    this.#backend = backend; this.#crypto = crypto; this.#context = copyContext(context); this.#recordLimit = recordLimit; this.#byteLimit = byteLimit;
  }
  static async open(
    backend: StrictBrowserOutboxBackend, context: BrowserHostOutboxContextV1, keys: BrowserHostOutboxKeyRingV1,
    limits: { readonly records?: number; readonly bytes?: number; readonly crypto?: Crypto; readonly random?: BrowserRandomBytes } = {},
  ): Promise<BrowserHostOutboxV1> {
    const recordLimit = limits.records ?? MAX_RECORDS; const byteLimit = limits.bytes ?? MAX_TOTAL_BYTES;
    if (!Number.isInteger(recordLimit) || recordLimit < 1 || recordLimit > MAX_RECORDS || !Number.isInteger(byteLimit) || byteLimit < 1 || byteLimit > MAX_TOTAL_BYTES) throw new BrowserOutboxError("HOST_OUTBOX_UNAVAILABLE", "browser outbox limits are invalid");
    let crypto: BrowserXChaCha20Poly1305;
    try { crypto = new BrowserXChaCha20Poly1305(context, keys, limits.crypto, limits.random); } catch { throw new BrowserOutboxError("HOST_OUTBOX_UNAVAILABLE", "browser outbox crypto is unavailable"); }
    const outbox = new BrowserHostOutboxV1(backend, crypto, context, recordLimit, byteLimit);
    let rows: readonly BrowserOutboxEncryptedRow[]; try { rows = await backend.load(); } catch { throw new BrowserOutboxError("HOST_OUTBOX_UNAVAILABLE", "strict IndexedDB is unavailable"); }
    if (rows.length > recordLimit) throw new BrowserOutboxError("HOST_OUTBOX_CORRUPT", "record bound exceeded");
    const operationGenerations = new Set<string>();
    for (const row of rows) {
      try {
        if (!/^[0-9a-f]{32}$/.test(row.id) || !Number.isSafeInteger(row.keyVersion) || row.keyVersion < 0 || row.ciphertext.length > MAX_RECORD_BYTES) throw new Error();
        const id = fromHex(row.id); const plaintext = await crypto.open(id, row.keyVersion, row.ciphertext.slice()); const record = decodeRecord(plaintext);
        if (toHex(recordId(record)) !== row.id || recordKeyVersion(record) !== row.keyVersion || outbox.#records.has(row.id)) throw new Error();
        if (record.kind === 0) {
          const validated = await outbox.#validateEntry(record.entry, row.keyVersion, record.operationCode);
          if (validated.operationIdRequired && !operationGenerations.add(operationGeneration(record.entry))) throw new Error();
        }
        outbox.#records.set(row.id, { record, encryptedBytes: row.ciphertext.length });
      } catch {
        try { await backend.quarantineStrict(row); } catch { throw new BrowserOutboxError("HOST_OUTBOX_UNAVAILABLE", "corrupt browser outbox quarantine failed"); }
        throw new BrowserOutboxError("HOST_OUTBOX_CORRUPT", "authenticated browser outbox record is corrupt");
      }
    }
    if (outbox.#totalBytes() > byteLimit) throw new BrowserOutboxError("HOST_OUTBOX_CORRUPT", "encrypted browser outbox byte bound exceeded");
    return outbox;
  }
  get contextBinding(): BrowserHostOutboxContextV1 { return copyContext(this.#context); }
  get activeKeyVersion(): number { return this.#crypto.activeKeyVersion; }

  async prepare(input: BrowserPrepareOutboxV1): Promise<BrowserOutboxRetryV1> {
    const entry = cloneEntry(input.entry);
    const outboxId = toHex(entry[1]);
    if (entry[2] !== 0 || this.#records.has(outboxId) || this.#preparingOutboxIds.has(outboxId) || entry[20] !== this.#crypto.activeKeyVersion) throw new BrowserOutboxError("HOST_OUTBOX_STATE_INVALID", "browser outbox entry is not new Prepared with a unique outbox ID and the active key");
    this.#preparingOutboxIds.add(outboxId);
    let reservedOperation: string | undefined;
    try {
      const validated = await this.#validateEntry(entry, this.#crypto.activeKeyVersion);
      if (validated.operationIdRequired) {
        reservedOperation = operationGeneration(entry);
        if (this.#preparingOperationGenerations.has(reservedOperation) || this.#hasLiveOperationGeneration(reservedOperation)) throw new BrowserOutboxError("HOST_OUTBOX_STATE_INVALID", "browser outbox operation generation is already live");
        this.#preparingOperationGenerations.add(reservedOperation);
      }
      const record: LiveRecord = { kind: 0, entry, state: 0, terminal: false, recoverUntil: BigInt(entry[19]), operationCode: validated.operationCode };
      await this.#commit(record, true); return retry(record);
    } finally {
      this.#preparingOutboxIds.delete(outboxId);
      if (reservedOperation) this.#preparingOperationGenerations.delete(reservedOperation);
    }
  }
  retry(outboxId: Uint8Array, finalized: bigint): BrowserOutboxRetryV1 {
    const record = this.#live(outboxId); if (finalized >= record.recoverUntil) throw new BrowserOutboxError("HOST_OUTBOX_EXPIRED", "browser outbox recovery window closed");
    if (record.state !== 0 && record.state !== 1) throw new BrowserOutboxError("HOST_OUTBOX_STATE_INVALID", "browser outbox request is not retryable"); return retry(record);
  }
  async markSent(outboxId: Uint8Array): Promise<void> {
    const record = this.#live(outboxId); if (record.state !== 0) throw new BrowserOutboxError("HOST_OUTBOX_STATE_INVALID", "request is not Prepared"); await this.#commit({ ...record, state: 1 }, false);
  }
  async prepareCancel(outboxId: Uint8Array, exactCancel: Uint8Array, nextSequence: number): Promise<BrowserOutboxRetryV1> {
    const record = this.#live(outboxId); if (record.state !== 0 && record.state !== 1) throw new BrowserOutboxError("HOST_OUTBOX_STATE_INVALID", "request cannot be cancelled");
    const event = decodeHostV2("CancelledEventV2", exactCancel).value;
    if (!equal(event[1], record.entry[6]) || Number(event[2]) !== nextSequence || event[3] !== 4) throw new BrowserOutboxError("HOST_OUTBOX_BINDING_INVALID", "cancel event is not bound to the live request cursor");
    const entry = cloneEntry({ ...record.entry, 2: 0, 3: exactCancel.slice(), 5: await this.#fingerprint(exactCancel, record.entry[4]), 9: nextSequence, 15: 4, 20: this.#crypto.activeKeyVersion });
    const cancelled: LiveRecord = { kind: 0, entry, state: 0, terminal: false, recoverUntil: record.recoverUntil, operationCode: record.operationCode };
    await this.#commit(cancelled, false); return retry(cancelled);
  }
  async installTerminal(outboxId: Uint8Array, response: Uint8Array, terminalBlock: bigint): Promise<{ readonly responseHash: Uint8Array; readonly ack: Uint8Array }> {
    const record = this.#live(outboxId); const event = decodeHostV2("EventV2", response).value;
    if (record.state === 2) { if (!equal(record.response!, response)) throw new BrowserOutboxError("HOST_OUTBOX_STATE_INVALID", "terminal response changed"); return { responseHash: record.responseHash!.slice(), ack: record.responseAck!.slice() }; }
    if ((record.state !== 0 && record.state !== 1) || ![2, 3, 4].includes(event[3])
      || (event[3] !== 3 && event[3] !== record.entry[15]) || !equal(event[1], record.entry[6])) throw new BrowserOutboxError("HOST_OUTBOX_BINDING_INVALID", "terminal response is not bound to the durable request");
    const responseHash = await this.#crypto.digest(response); const ack = encodeHostV2("ResponseAckV1", { 0: record.entry[6], 1: record.entry[7], 2: record.entry[8], 3: responseHash });
    const installed: LiveRecord = { ...record, state: 2, response: response.slice(), responseAck: ack, responseHash: responseHash.slice(), terminal: true, recoverUntil: terminalBlock + RECOVERY_BLOCKS };
    await this.#commit(installed, false); return { responseHash: responseHash.slice(), ack: ack.slice() };
  }
  installedAck(outboxId: Uint8Array): { readonly responseHash: Uint8Array; readonly ack: Uint8Array } {
    const record = this.#live(outboxId); if (record.state !== 2) throw new BrowserOutboxError("HOST_OUTBOX_STATE_INVALID", "response acknowledgement is absent"); return { responseHash: record.responseHash!.slice(), ack: record.responseAck!.slice() };
  }
  async confirmAck(outboxId: Uint8Array, responseHash: Uint8Array): Promise<void> {
    const loaded = this.#get(outboxId); if (loaded.record.kind === 1 && loaded.record.state === 3 && equal(loaded.record.responseHash!, responseHash)) return;
    if (loaded.record.kind !== 0 || loaded.record.state !== 2 || !equal(loaded.record.responseHash!, responseHash)) throw new BrowserOutboxError("HOST_OUTBOX_STATE_INVALID", "response acknowledgement mismatches");
    const tombstone: TombstoneRecord = { kind: 1, outboxId: loaded.record.entry[1].slice(), state: 3, responseHash: responseHash.slice(), requestFingerprint: loaded.record.entry[5].slice(), terminal: loaded.record.terminal, recoverUntil: loaded.record.recoverUntil, keyVersion: this.#crypto.activeKeyVersion };
    await this.#commit(tombstone, false);
  }
  async expire(outboxId: Uint8Array, finalized: bigint): Promise<void> {
    const loaded = this.#get(outboxId); if (loaded.record.kind === 1 || finalized < loaded.record.recoverUntil) throw new BrowserOutboxError("HOST_OUTBOX_STATE_INVALID", "browser outbox cannot expire yet");
    const tombstone: TombstoneRecord = { kind: 1, outboxId: loaded.record.entry[1].slice(), state: 4, requestFingerprint: loaded.record.entry[5].slice(), ...(loaded.record.responseHash ? { responseHash: loaded.record.responseHash.slice() } : {}), terminal: loaded.record.terminal, recoverUntil: loaded.record.recoverUntil, keyVersion: this.#crypto.activeKeyVersion };
    await this.#commit(tombstone, false);
  }
  async gc(finalized: bigint, limit: number): Promise<number> {
    const eligible = [...this.#records.entries()].filter(([, loaded]) => loaded.record.kind === 1 && finalized >= loaded.record.recoverUntil && (loaded.record.state === 4 || (loaded.record.state === 3 && loaded.record.terminal))).slice(0, Math.max(0, limit));
    for (const [id] of eligible) { try { await this.#backend.deleteStrict(id); } catch { throw new BrowserOutboxError("HOST_OUTBOX_UNAVAILABLE", "strict IndexedDB GC commit failed"); } this.#records.delete(id); } return eligible.length;
  }
  async digest(bytes: Uint8Array): Promise<Uint8Array> { return this.#crypto.digest(bytes); }
  async verifyProviderAck(publicKey: Uint8Array, message: Uint8Array, signature: Uint8Array): Promise<boolean> { return this.#crypto.verifyEd25519(publicKey, message, signature); }

  async #validateEntry(entry: HostOutboxEntryV1, keyVersion: number, expectedOperationCode?: number): Promise<ValidatedEntry> {
    if (!equal(entry[10], this.#context.registryHash) || !equal(entry[11], this.#context.genesisHash) || !equal(entry[12], this.#context.negotiatedTuple)
      || !equal(entry[13], this.#context.providerId) || !equal(entry[14], this.#context.providerEndpointHash) || entry[20] !== keyVersion) throw new BrowserOutboxError("HOST_OUTBOX_BINDING_INVALID", "browser outbox chain, tuple, provider, endpoint, or key binding mismatched");
    if (![2, 3, 4].includes(Number(entry[15]))) throw new BrowserOutboxError("HOST_OUTBOX_BINDING_INVALID", "expected terminal result kind is not exact");
    const created = BigInt(entry[17]); const authorityUntil = BigInt(entry[18]); const recoverUntil = BigInt(entry[19]);
    if (authorityUntil < created || authorityUntil > created + AUTHORITY_BLOCKS || recoverUntil < created + RECOVERY_BLOCKS || recoverUntil > created + MAX_RECOVERY_BLOCKS) throw new BrowserOutboxError("HOST_OUTBOX_BINDING_INVALID", "browser outbox authority or recovery bounds are invalid");
    if (!equal(entry[5], await this.#fingerprint(entry[3], entry[4]))) throw new BrowserOutboxError("HOST_OUTBOX_BINDING_INVALID", "browser outbox request fingerprint mismatched");
    let request: Record<number, unknown>;
    try { request = decodeHostV2("RequestV2", entry[3]).value as Record<number, unknown>; }
    catch {
      const cancel = decodeHostV2("CancelledEventV2", entry[3]).value;
      if (!equal(cancel[1], entry[6]) || Number(cancel[2]) !== Number(entry[9]) || cancel[3] !== 4 || entry[15] !== 4) throw new BrowserOutboxError("HOST_OUTBOX_BINDING_INVALID", "durable cancel binding mismatched");
      const operation = Object.values(HOST_V2_OPERATION_BINDINGS).find(({ code }) => code === expectedOperationCode);
      if (!operation) throw new BrowserOutboxError("HOST_OUTBOX_BINDING_INVALID", "durable cancel lost its exact operation binding");
      return { operationCode: operation.code, operationIdRequired: operation.operationIdRequired };
    }
    const operationCode = Number(request[3]);
    const operation = Object.values(HOST_V2_OPERATION_BINDINGS).find(({ code }) => code === operationCode);
    if (!operation || !PROVIDER_BYTE_OPERATION_CODES.has(operationCode)
      || (expectedOperationCode !== undefined && operationCode !== expectedOperationCode)) throw new BrowserOutboxError("HOST_OUTBOX_BINDING_INVALID", "request is not a provider-byte operation or its code mismatched");
    if (!(request[1] instanceof Uint8Array) || !equal(request[1], entry[6]) || Number(request[7]) !== Number(entry[8])) throw new BrowserOutboxError("HOST_OUTBOX_BINDING_INVALID", "request ID or generation mismatched");
    const operationId = request[5];
    if (operation.operationIdRequired
      ? !(operationId instanceof Uint8Array) || !equal(operationId, entry[7])
      : operationId !== undefined || entry[7].some((byte) => byte !== 0)) throw new BrowserOutboxError("HOST_OUTBOX_BINDING_INVALID", "request operation ID presence or value mismatched");
    let authority: Record<number, unknown>;
    try {
      authority = decodeHostV2("ProviderCapabilityV1", entry[4]).value as Record<number, unknown>;
      if (!equal(authority[1] as Uint8Array, entry[10]) || !equal(authority[2] as Uint8Array, entry[11])
        || !(request[4] instanceof Uint8Array) || !equal(authority[3] as Uint8Array, request[4]) || authority[5] !== request[2]
        || !equal(authority[8] as Uint8Array, entry[13]) || !(authority[9] as unknown[]).includes(operationCode)
        || Number(authority[12]) !== Number(entry[17]) || Number(authority[13]) !== Number(entry[18])) throw new Error();
    } catch {
      try {
        authority = decodeHostV2("ResumeTokenV1", entry[4]).value as Record<number, unknown>;
        if (!equal(authority[1] as Uint8Array, entry[10]) || !equal(authority[2] as Uint8Array, entry[11])
          || !equal(authority[3] as Uint8Array, entry[13]) || !equal(authority[5] as Uint8Array, entry[7])
          || Number(authority[9]) !== Number(entry[9]) || Number(authority[10]) !== Number(entry[8])
          || Number(authority[11]) !== Number(entry[17]) || Number(authority[12]) !== Number(entry[18])) throw new Error();
      } catch { throw new BrowserOutboxError("HOST_OUTBOX_BINDING_INVALID", "authority is not bound to request and provider"); }
    }
    return { operationCode, operationIdRequired: operation.operationIdRequired };
  }
  async #fingerprint(request: Uint8Array, authority: Uint8Array): Promise<Uint8Array> { const bytes = new Uint8Array(request.length + authority.length); bytes.set(request); bytes.set(authority, request.length); return this.#crypto.digest(bytes); }
  async #commit(record: DurableRecord, create: boolean): Promise<void> {
    if (record.kind === 0 && record.entry[20] !== this.#crypto.activeKeyVersion) record = { ...record, entry: cloneEntry({ ...record.entry, 20: this.#crypto.activeKeyVersion }) };
    if (record.kind === 1 && record.keyVersion !== this.#crypto.activeKeyVersion) record = { ...record, keyVersion: this.#crypto.activeKeyVersion };
    const idBytes = recordId(record); const id = toHex(idBytes); const plaintext = encodeRecord(record);
    let sealed: { readonly keyVersion: number; readonly ciphertext: Uint8Array }; try { sealed = await this.#crypto.seal(idBytes, plaintext); } catch { throw new BrowserOutboxError("HOST_OUTBOX_UNAVAILABLE", "browser outbox encryption failed"); }
    if (sealed.ciphertext.length > MAX_RECORD_BYTES) throw new BrowserOutboxError("HOST_OUTBOX_FULL", "encrypted browser outbox record is too large");
    await this.#withCommitCapacity(async () => {
      const previous = this.#records.get(id);
      if ((create && (previous || this.#records.size >= this.#recordLimit))
        || this.#totalBytes() - (previous?.encryptedBytes ?? 0) + sealed.ciphertext.length > this.#byteLimit) {
        throw new BrowserOutboxError("HOST_OUTBOX_FULL", "browser outbox capacity is full");
      }
      const row = { id, keyVersion: sealed.keyVersion, ciphertext: sealed.ciphertext.slice() };
      try { await this.#backend.putStrict(row); }
      catch { throw new BrowserOutboxError("HOST_OUTBOX_UNAVAILABLE", "strict IndexedDB commit failed"); }
      this.#records.set(id, { record, encryptedBytes: row.ciphertext.length });
    });
  }
  async #withCommitCapacity<T>(operation: () => Promise<T>): Promise<T> {
    const previous = this.#commitTail;
    let release!: () => void;
    this.#commitTail = new Promise<void>((resolve) => { release = resolve; });
    await previous;
    try { return await operation(); }
    finally { release(); }
  }
  #get(outboxId: Uint8Array): LoadedRecord { const loaded = this.#records.get(toHex(outboxId)); if (!loaded) throw new BrowserOutboxError("HOST_OUTBOX_STATE_INVALID", "browser outbox entry is absent"); return loaded; }
  #live(outboxId: Uint8Array): LiveRecord { const record = this.#get(outboxId).record; if (record.kind !== 0) throw new BrowserOutboxError("HOST_OUTBOX_STATE_INVALID", "browser outbox authority is retired"); return record; }
  #hasLiveOperationGeneration(identity: string): boolean {
    for (const loaded of this.#records.values()) {
      if (loaded.record.kind === 0 && operationRequiresId(loaded.record.operationCode) && operationGeneration(loaded.record.entry) === identity) return true;
    }
    return false;
  }
  #totalBytes(): number { let total = 0; for (const loaded of this.#records.values()) total += loaded.encryptedBytes; return total; }
}

export function providerAckConfirmationMessage(context: BrowserHostOutboxContextV1, outboxId: Uint8Array, responseHash: Uint8Array): Uint8Array {
  return encodeHostV2Value({ 0: 1, 1: outboxId, 2: responseHash, 3: context.providerId, 4: context.providerEndpointHash, 5: context.negotiatedTuple });
}
function retry(record: LiveRecord): BrowserOutboxRetryV1 { return { request: record.entry[3].slice(), authority: record.entry[4].slice(), requestId: record.entry[6].slice(), operationId: record.entry[7].slice(), outboxId: record.entry[1].slice(), fingerprint: record.entry[5].slice(), expectedResponseKind: Number(record.entry[15]), intendedCursor: Number(record.entry[9]), cancel: isCancel(record.entry[3]), operationCode: record.operationCode }; }
function isCancel(bytes: Uint8Array): boolean { try { decodeHostV2("CancelledEventV2", bytes); return true; } catch { return false; } }
function cloneEntry(entry: HostOutboxEntryV1): HostOutboxEntryV1 { return decodeHostV2("HostOutboxEntryV1", encodeHostV2("HostOutboxEntryV1", entry)).value; }
function recordId(record: DurableRecord): Uint8Array { return record.kind === 0 ? record.entry[1] : record.outboxId; }
function recordKeyVersion(record: DurableRecord): number { return record.kind === 0 ? Number(record.entry[20]) : record.keyVersion; }
function encodeRecord(record: DurableRecord): Uint8Array {
  const map: HostV2Map = record.kind === 0 ? { 0: 1, 1: 0, 2: encodeHostV2("HostOutboxEntryV1", record.entry), 3: record.state, 7: record.terminal, 8: record.recoverUntil, 12: record.operationCode, ...(record.response ? { 4: record.response } : {}), ...(record.responseAck ? { 5: record.responseAck } : {}), ...(record.responseHash ? { 6: record.responseHash } : {}) }
    : { 0: 1, 1: 1, 3: record.state, 7: record.terminal, 8: record.recoverUntil, 9: record.outboxId, 10: record.keyVersion, 11: record.requestFingerprint, ...(record.responseHash ? { 6: record.responseHash } : {}) };
  return encodeHostV2Value(map);
}
function decodeRecord(bytes: Uint8Array): DurableRecord {
  const value = decodeCanonicalHostV2Value(bytes); if (typeof value !== "object" || value === null || Array.isArray(value) || value instanceof Uint8Array) throw new Error(); const map = value as HostV2Map;
  const keys = Object.keys(map).map(Number).sort((a, b) => a - b); if (map[0] !== 1 || (map[1] !== 0 && map[1] !== 1) || typeof map[3] !== "number" || typeof map[7] !== "boolean" || (typeof map[8] !== "number" && typeof map[8] !== "bigint")) throw new Error();
  if (map[1] === 0) {
    if (keys.some((key) => ![0, 1, 2, 3, 4, 5, 6, 7, 8, 12].includes(key)) || ![0, 1, 2, 3, 7, 8, 12].every((key) => keys.includes(key)) || !(map[2] instanceof Uint8Array) || ![0, 1, 2].includes(map[3]) || typeof map[12] !== "number") throw new Error();
    const state = map[3] as 0 | 1 | 2; const response = optionalBytes(map[4]); const ack = optionalBytes(map[5]); const hash = optionalBytes(map[6], 32); if ((state === 2) !== Boolean(response && ack && hash) || (state === 2) !== map[7]) throw new Error();
    return { kind: 0, entry: decodeHostV2("HostOutboxEntryV1", map[2]).value, state, ...(response ? { response } : {}), ...(ack ? { responseAck: ack } : {}), ...(hash ? { responseHash: hash } : {}), terminal: map[7], recoverUntil: BigInt(map[8] as number | bigint), operationCode: map[12] };
  }
  if (keys.some((key) => ![0, 1, 3, 6, 7, 8, 9, 10, 11].includes(key)) || ![0, 1, 3, 7, 8, 9, 10, 11].every((key) => keys.includes(key)) || ![3, 4].includes(map[3]) || !(map[9] instanceof Uint8Array) || map[9].length !== 16 || typeof map[10] !== "number" || !(map[11] instanceof Uint8Array) || map[11].length !== 32) throw new Error();
  const responseHash = optionalBytes(map[6], 32);
  if (map[3] === 3 && (!responseHash || map[7] !== true)) throw new Error();
  return { kind: 1, outboxId: map[9].slice(), state: map[3] as 3 | 4, ...(responseHash ? { responseHash } : {}), requestFingerprint: map[11].slice(), terminal: map[7], recoverUntil: BigInt(map[8] as number | bigint), keyVersion: map[10] };
}
function optionalBytes(value: unknown, length?: number): Uint8Array | undefined { if (value === undefined) return undefined; if (!(value instanceof Uint8Array) || (length !== undefined && value.length !== length)) throw new Error(); return value.slice(); }
function equal(left: Uint8Array, right: Uint8Array): boolean { return left.length === right.length && left.every((byte, index) => byte === right[index]); }
function toHex(bytes: Uint8Array): string { return [...bytes].map((byte) => byte.toString(16).padStart(2, "0")).join(""); }
function fromHex(value: string): Uint8Array { return Uint8Array.from(value.match(/../g)!.map((byte) => Number.parseInt(byte, 16))); }
function operationGeneration(entry: HostOutboxEntryV1): string { return `${toHex(entry[7])}:${BigInt(entry[8]).toString(10)}`; }
function operationRequiresId(operationCode: number): boolean { return Object.values(HOST_V2_OPERATION_BINDINGS).some(({ code, operationIdRequired }) => code === operationCode && operationIdRequired); }
