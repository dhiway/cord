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
import { blake2b256 } from "@cord-network/origin-sdk-crypto";
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
export interface BrowserPrepareOutboxV1 { readonly entry: HostOutboxEntryV1; readonly uploadChunks?: readonly Uint8Array[] }
export interface BrowserOutboxRetryV1 {
  readonly request: Uint8Array; readonly authority: Uint8Array; readonly requestId: Uint8Array;
  readonly operationId: Uint8Array; readonly outboxId: Uint8Array; readonly fingerprint: Uint8Array;
  readonly expectedResponseKind: number; readonly intendedCursor: number; readonly cancel: boolean;
  readonly operationCode: number;
  readonly uploadChunk?: Uint8Array;
}
export interface BrowserRecoveredSuccessorV1 {
  readonly predecessorOutboxId: Uint8Array; readonly cursor: number; readonly hostKeyId: Uint8Array;
}
interface LiveRecord {
  readonly kind: 0; readonly entry: HostOutboxEntryV1; readonly state: 0 | 1 | 2 | 3;
  readonly response?: Uint8Array; readonly responseAck?: Uint8Array; readonly responseHash?: Uint8Array;
  readonly successorAuthority?: Uint8Array; readonly successorCursor?: number;
  readonly predecessorOutboxId?: Uint8Array; readonly predecessorResponseHash?: Uint8Array;
  readonly successorOutboxId?: Uint8Array; readonly retiredSuccessorOutboxId?: Uint8Array;
  readonly terminal: boolean; readonly recoverUntil: bigint; readonly operationCode: number;
  readonly uploadSpoolOutboxId?: Uint8Array; readonly uploadChunkIds?: readonly Uint8Array[];
  readonly uploadChunkId?: Uint8Array; readonly uploadSpoolRetired?: boolean;
}
interface TombstoneRecord {
  readonly kind: 1; readonly outboxId: Uint8Array; readonly state: 3 | 4;
  readonly responseHash?: Uint8Array; readonly requestFingerprint: Uint8Array;
  readonly terminal: boolean; readonly recoverUntil: bigint; readonly keyVersion: number;
  readonly uploadSpoolOutboxId?: Uint8Array; readonly uploadSpoolRetired?: boolean;
}
interface UploadChunkRecord {
  readonly kind: 2; readonly chunkId: Uint8Array; readonly spoolOutboxId: Uint8Array;
  readonly operationId: Uint8Array; readonly index: number; readonly exactChunk: Uint8Array; readonly keyVersion: number;
}
type DurableRecord = LiveRecord | TombstoneRecord | UploadChunkRecord;
interface LoadedRecord { readonly record: DurableRecord; readonly encryptedBytes: number; readonly encryptedRow: BrowserOutboxEncryptedRow }
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
        outbox.#records.set(row.id, { record, encryptedBytes: row.ciphertext.length, encryptedRow: copyRow(row) });
      } catch {
        try { await backend.quarantineStrict(row); } catch { throw new BrowserOutboxError("HOST_OUTBOX_UNAVAILABLE", "corrupt browser outbox quarantine failed"); }
        throw new BrowserOutboxError("HOST_OUTBOX_CORRUPT", "authenticated browser outbox record is corrupt");
      }
    }
    try { await outbox.#validateSuccessorLinks(); }
    catch { throw new BrowserOutboxError("HOST_OUTBOX_CORRUPT", "authenticated browser outbox successor link is corrupt"); }
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
      const chunks = await this.#prepareUploadChunks(entry, validated.operationCode, input.uploadChunks);
      const record: LiveRecord = {
        kind: 0, entry, state: 0, terminal: false, recoverUntil: BigInt(entry[19]), operationCode: validated.operationCode,
        ...(chunks.length > 0 ? { uploadSpoolOutboxId: entry[1].slice(), uploadChunkIds: chunks.map((chunk) => chunk.chunkId.slice()) } : {}),
      };
      if (chunks.length > 0) await this.#commitPreparedUpload(record, chunks);
      else await this.#commit(record, true);
      return retry(record, this.#records);
    } finally {
      this.#preparingOutboxIds.delete(outboxId);
      if (reservedOperation) this.#preparingOperationGenerations.delete(reservedOperation);
    }
  }
  retry(outboxId: Uint8Array, finalized: bigint): BrowserOutboxRetryV1 {
    const record = this.#live(outboxId); if (finalized >= record.recoverUntil) throw new BrowserOutboxError("HOST_OUTBOX_EXPIRED", "browser outbox recovery window closed");
    if (record.state !== 0 && record.state !== 1) throw new BrowserOutboxError("HOST_OUTBOX_STATE_INVALID", "browser outbox request is not retryable"); return retry(record, this.#records);
  }
  linkedSuccessor(predecessorId: Uint8Array, finalized: bigint): BrowserOutboxRetryV1 | undefined {
    const predecessor = this.#live(predecessorId);
    if (!predecessor.successorOutboxId) return undefined;
    const successor = this.#live(predecessor.successorOutboxId);
    if (!successor.predecessorOutboxId || !equal(successor.predecessorOutboxId, predecessor.entry[1])) {
      throw new BrowserOutboxError("HOST_OUTBOX_STATE_INVALID", "linked successor lookup is misbound");
    }
    if (finalized >= successor.recoverUntil) throw new BrowserOutboxError("HOST_OUTBOX_EXPIRED", "linked successor recovery window closed");
    if (successor.state !== 0 && successor.state !== 1) throw new BrowserOutboxError("HOST_OUTBOX_STATE_INVALID", "linked successor is not retryable");
    return retry(successor, this.#records);
  }
  recoverSuccessor(exactRequest: Uint8Array, exactResumeToken: Uint8Array): BrowserRecoveredSuccessorV1 {
    let recovered: BrowserRecoveredSuccessorV1 | undefined;
    for (const loaded of this.#records.values()) {
      const record = loaded.record;
      if (record.kind !== 0 || record.state !== 3 || record.terminal || !record.successorAuthority
        || record.successorCursor === undefined || !equal(record.entry[3], exactRequest)
        || !equal(record.successorAuthority, exactResumeToken)) continue;
      if (recovered) throw new BrowserOutboxError("HOST_OUTBOX_STATE_INVALID", "provider successor authority is not unique");
      const token = decodeHostV2("ResumeTokenV1", record.successorAuthority).value;
      recovered = {
        predecessorOutboxId: record.entry[1].slice(), cursor: record.successorCursor,
        hostKeyId: optionalBytes(token[4], 32)!,
      };
    }
    if (!recovered) throw new BrowserOutboxError("HOST_OUTBOX_STATE_INVALID", "exact durable provider successor is absent");
    return recovered;
  }
  async markSent(outboxId: Uint8Array): Promise<void> {
    const record = this.#live(outboxId); if (record.state !== 0) throw new BrowserOutboxError("HOST_OUTBOX_STATE_INVALID", "request is not Prepared"); await this.#commit({ ...record, state: 1 }, false);
  }
  async prepareCancel(outboxId: Uint8Array, exactCancel: Uint8Array, nextSequence: number): Promise<BrowserOutboxRetryV1> {
    const record = this.#live(outboxId); if (record.state !== 0 && record.state !== 1) throw new BrowserOutboxError("HOST_OUTBOX_STATE_INVALID", "request cannot be cancelled");
    const event = decodeHostV2("CancelledEventV2", exactCancel).value;
    if (!equal(event[1], record.entry[6]) || Number(event[2]) !== nextSequence || event[3] !== 4) throw new BrowserOutboxError("HOST_OUTBOX_BINDING_INVALID", "cancel event is not bound to the live request cursor");
    const entry = cloneEntry({ ...record.entry, 2: 0, 3: exactCancel.slice(), 5: await this.#fingerprint(exactCancel, record.entry[4]), 9: nextSequence, 15: 4, 20: this.#crypto.activeKeyVersion });
    const cancelled: LiveRecord = { ...record, entry, state: 0, terminal: false };
    await this.#commit(cancelled, false); return retry(cancelled, this.#records);
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
  async installSuccessor(
    outboxId: Uint8Array, response: Uint8Array, exactResumeToken: Uint8Array, cursor: number,
  ): Promise<{ readonly responseHash: Uint8Array; readonly ack: Uint8Array }> {
    const record = this.#live(outboxId); const event = decodeHostV2("EventV2", response).value;
    if (record.state === 2) {
      if (!equal(record.response!, response) || !equal(record.successorAuthority!, exactResumeToken) || record.successorCursor !== cursor || record.terminal) {
        throw new BrowserOutboxError("HOST_OUTBOX_STATE_INVALID", "installed successor response changed");
      }
      return { responseHash: record.responseHash!.slice(), ack: record.responseAck!.slice() };
    }
    if ((record.state !== 0 && record.state !== 1) || ![0, 1].includes(Number(event[3])) || !equal(event[1], record.entry[6])
      || !Number.isSafeInteger(Number(event[2])) || Number(event[2]) + 1 !== cursor) {
      throw new BrowserOutboxError("HOST_OUTBOX_BINDING_INVALID", "successor response is not a live accepted/progress event");
    }
    const token = decodeHostV2("ResumeTokenV1", exactResumeToken).value;
    const request = decodeHostV2("RequestV2", record.entry[3]).value as Record<number, unknown>;
    const payload = request[8] as Record<number, unknown>; const hostKeyId = authorityHostKey(record.entry[4]);
    if (!Number.isSafeInteger(cursor) || cursor < 0 || !equal(token[1], record.entry[10]) || !equal(token[2], record.entry[11])
      || !equal(token[3], record.entry[13]) || !equal(token[4], hostKeyId) || !equal(token[5], record.entry[7]) || Number(token[9]) + 1 !== cursor
      || !equal(token[6], payload[0] as Uint8Array) || token[7] !== payload[1] || BigInt(token[8]) !== BigInt(payload[2] as number | bigint)
      || BigInt(token[10]) !== BigInt(record.entry[8]) + 1n || token[14] !== false) {
      throw new BrowserOutboxError("HOST_OUTBOX_BINDING_INVALID", "resume token is not the exact active successor");
    }
    const issued = BigInt(token[11]); const expires = BigInt(token[12]);
    if (expires < issued || expires > issued + AUTHORITY_BLOCKS) throw new BrowserOutboxError("HOST_OUTBOX_BINDING_INVALID", "resume token lifetime is invalid");
    const responseHash = await this.#crypto.digest(response);
    const ack = encodeHostV2("ResponseAckV1", { 0: record.entry[6], 1: record.entry[7], 2: record.entry[8], 3: responseHash });
    const installed: LiveRecord = {
      ...record, state: 2, response: response.slice(), responseAck: ack, responseHash: responseHash.slice(),
      successorAuthority: exactResumeToken.slice(), successorCursor: cursor, terminal: false,
    };
    await this.#commit(installed, false); return { responseHash: responseHash.slice(), ack: ack.slice() };
  }
  installedAck(outboxId: Uint8Array): { readonly responseHash: Uint8Array; readonly ack: Uint8Array } {
    const record = this.#live(outboxId); if (record.state !== 2 && record.state !== 3) throw new BrowserOutboxError("HOST_OUTBOX_STATE_INVALID", "response acknowledgement is absent"); return { responseHash: record.responseHash!.slice(), ack: record.responseAck!.slice() };
  }
  async confirmAck(outboxId: Uint8Array, responseHash: Uint8Array): Promise<void> {
    const loaded = this.#get(outboxId); if (loaded.record.kind === 1 && loaded.record.state === 3 && equal(loaded.record.responseHash!, responseHash)) return;
    if (loaded.record.kind !== 0 || (loaded.record.state !== 2 && loaded.record.state !== 3) || !equal(loaded.record.responseHash!, responseHash)) throw new BrowserOutboxError("HOST_OUTBOX_STATE_INVALID", "response acknowledgement mismatches");
    if (!loaded.record.terminal) {
      if (!loaded.record.successorAuthority || loaded.record.successorCursor === undefined) throw new BrowserOutboxError("HOST_OUTBOX_STATE_INVALID", "nonterminal acknowledgement has no exact successor");
      if (loaded.record.state === 3) return;
      await this.#commit({ ...loaded.record, state: 3 }, false); return;
    }
    const tombstone: TombstoneRecord = {
      kind: 1, outboxId: loaded.record.entry[1].slice(), state: 3, responseHash: responseHash.slice(), requestFingerprint: loaded.record.entry[5].slice(),
      terminal: loaded.record.terminal, recoverUntil: loaded.record.recoverUntil, keyVersion: this.#crypto.activeKeyVersion,
      ...(loaded.record.uploadSpoolOutboxId ? { uploadSpoolOutboxId: loaded.record.uploadSpoolOutboxId.slice() } : {}),
    };
    if (loaded.record.predecessorOutboxId) { await this.#retireLinkedSuccessor(loaded, tombstone); return; }
    await this.#commit(tombstone, false);
  }
  async prepareSuccessor(predecessorId: Uint8Array, input: BrowserPrepareOutboxV1): Promise<BrowserOutboxRetryV1> {
    const predecessorLoaded = this.#get(predecessorId); const predecessor = predecessorLoaded.record;
    if (predecessor.kind !== 0 || predecessor.state !== 3 || predecessor.terminal || !predecessor.successorAuthority || predecessor.successorCursor === undefined || !predecessor.responseHash) {
      throw new BrowserOutboxError("HOST_OUTBOX_STATE_INVALID", "predecessor is not an acknowledged resumable response");
    }
    const entry = cloneEntry(input.entry); const successorId = toHex(entry[1]);
    const expectedGeneration = BigInt(predecessor.entry[8]) + 1n;
    if (!equal(entry[4], predecessor.successorAuthority) || !equal(entry[7], predecessor.entry[7])
      || BigInt(entry[8]) !== expectedGeneration || Number(entry[9]) !== predecessor.successorCursor
      || !equal(entry[10], predecessor.entry[10]) || !equal(entry[11], predecessor.entry[11])
      || !equal(entry[12], predecessor.entry[12]) || !equal(entry[13], predecessor.entry[13])
      || !equal(entry[14], predecessor.entry[14]) || Number(entry[15]) !== Number(predecessor.entry[15])) {
      throw new BrowserOutboxError("HOST_OUTBOX_BINDING_INVALID", "successor entry is not bound to its predecessor");
    }
    const existing = this.#records.get(successorId);
    if (existing) {
      if (predecessor.successorOutboxId && equal(predecessor.successorOutboxId, entry[1]) && existing.record.kind === 0
        && existing.record.predecessorOutboxId && equal(existing.record.predecessorOutboxId, predecessor.entry[1])
        && existing.record.predecessorResponseHash && equal(existing.record.predecessorResponseHash, predecessor.responseHash)
        && equal(encodeHostV2("HostOutboxEntryV1", existing.record.entry), encodeHostV2("HostOutboxEntryV1", entry))) return retry(existing.record, this.#records);
      throw new BrowserOutboxError("HOST_OUTBOX_STATE_INVALID", "successor outbox identity was replayed");
    }
    if (predecessor.successorOutboxId || predecessor.retiredSuccessorOutboxId) throw new BrowserOutboxError("HOST_OUTBOX_STATE_INVALID", "predecessor already consumed its successor authority");
    const identity = operationGeneration(entry);
    if (this.#preparingOperationGenerations.has(identity) || this.#hasLiveOperationGeneration(identity)) throw new BrowserOutboxError("HOST_OUTBOX_STATE_INVALID", "browser outbox operation generation is already live");
    this.#preparingOperationGenerations.add(identity);
    try {
      const validated = await this.#validateEntry(entry, this.#crypto.activeKeyVersion, predecessor.operationCode);
      const uploadChunkId = predecessor.uploadSpoolOutboxId ? this.#successorUploadChunk(predecessor, Number(entry[9])) : undefined;
      const successor: LiveRecord = {
        kind: 0, entry, state: 0, terminal: false, recoverUntil: BigInt(entry[19]), operationCode: validated.operationCode,
        predecessorOutboxId: predecessor.entry[1].slice(), predecessorResponseHash: predecessor.responseHash.slice(),
        ...(predecessor.uploadSpoolOutboxId ? {
          uploadSpoolOutboxId: predecessor.uploadSpoolOutboxId.slice(),
          ...(uploadChunkId ? { uploadChunkId } : {}),
        } : {}),
      };
      const updatedPredecessor: LiveRecord = { ...predecessor, successorOutboxId: entry[1].slice() };
      await this.#commitSuccessor(predecessorLoaded, updatedPredecessor, successor);
      return retry(successor, this.#records);
    } finally { this.#preparingOperationGenerations.delete(identity); }
  }
  async retireUploadSpool(terminalOutboxId: Uint8Array): Promise<void> {
    const terminalLoaded = this.#get(terminalOutboxId); const terminal = terminalLoaded.record;
    if (terminal.kind !== 1 || (terminal.state !== 4 && (terminal.state !== 3 || !terminal.terminal)) || !terminal.uploadSpoolOutboxId || terminal.uploadSpoolRetired) return;
    const rootId = toHex(terminal.uploadSpoolOutboxId); const rootLoaded = this.#records.get(rootId);
    if (!rootLoaded) throw new BrowserOutboxError("HOST_OUTBOX_STATE_INVALID", "terminal upload spool root is absent");
    const chunks = [...this.#records.entries()].filter(([, loaded]) => loaded.record.kind === 2 && equal(loaded.record.spoolOutboxId, terminal.uploadSpoolOutboxId!));
    const retiredTerminal: TombstoneRecord = { ...terminal, uploadSpoolRetired: true };
    const retiredRoot: DurableRecord = rootLoaded.record.kind === 0
      ? (() => { const { uploadChunkIds: _removed, ...root } = rootLoaded.record; return { ...root, uploadSpoolRetired: true }; })()
      : rootLoaded.record.kind === 1 && rootId === toHex(terminal.outboxId) ? retiredTerminal
      : (() => { throw new BrowserOutboxError("HOST_OUTBOX_STATE_INVALID", "terminal upload spool root is invalid"); })();
    let terminalRow: BrowserOutboxEncryptedRow; let rootRow: BrowserOutboxEncryptedRow;
    try { terminalRow = await this.#sealRecord(retiredTerminal); rootRow = rootId === terminalRow.id ? terminalRow : await this.#sealRecord(retiredRoot); }
    catch { throw new BrowserOutboxError("HOST_OUTBOX_UNAVAILABLE", "browser upload spool retirement encryption failed"); }
    await this.#withCommitCapacity(async () => {
      const expected: Record<string, BrowserOutboxEncryptedRow | null> = { [terminalRow.id]: terminalLoaded.encryptedRow };
      if (rootRow.id !== terminalRow.id) expected[rootRow.id] = rootLoaded.encryptedRow;
      for (const [id, loaded] of chunks) expected[id] = loaded.encryptedRow;
      try { await this.#backend.transactStrict({ expected, puts: rootRow.id === terminalRow.id ? [terminalRow] : [rootRow, terminalRow], deletes: chunks.map(([id]) => id) }); }
      catch { throw new BrowserOutboxError("HOST_OUTBOX_UNAVAILABLE", "strict upload spool retirement transaction failed"); }
      this.#records.set(terminalRow.id, { record: retiredTerminal, encryptedBytes: terminalRow.ciphertext.length, encryptedRow: copyRow(terminalRow) });
      if (rootRow.id !== terminalRow.id) this.#records.set(rootRow.id, { record: retiredRoot, encryptedBytes: rootRow.ciphertext.length, encryptedRow: copyRow(rootRow) });
      for (const [id] of chunks) this.#records.delete(id);
    });
  }
  async expire(outboxId: Uint8Array, finalized: bigint): Promise<void> {
    const loaded = this.#get(outboxId); const record = loaded.record;
    if (record.kind !== 0 || finalized < record.recoverUntil) throw new BrowserOutboxError("HOST_OUTBOX_STATE_INVALID", "browser outbox cannot expire yet");
    if (record.successorOutboxId) throw new BrowserOutboxError("HOST_OUTBOX_STATE_INVALID", "linked predecessor cannot expire before its successor retires");
    const tombstone: TombstoneRecord = { kind: 1, outboxId: record.entry[1].slice(), state: 4, requestFingerprint: record.entry[5].slice(), ...(record.responseHash ? { responseHash: record.responseHash.slice() } : {}), terminal: record.terminal, recoverUntil: record.recoverUntil, keyVersion: this.#crypto.activeKeyVersion, ...(record.uploadSpoolOutboxId ? { uploadSpoolOutboxId: record.uploadSpoolOutboxId.slice() } : {}) };
    if (record.predecessorOutboxId) await this.#retireLinkedSuccessor(loaded, tombstone);
    else await this.#commit(tombstone, false);
    await this.retireUploadSpool(tombstone.outboxId);
  }
  async gc(finalized: bigint, limit: number): Promise<number> {
    const linkedRetired = new Set([...this.#records.values()].flatMap(({ record }) => record.kind === 0 && record.retiredSuccessorOutboxId ? [toHex(record.retiredSuccessorOutboxId)] : []));
    const eligible = [...this.#records.entries()].filter(([id, loaded]) => loaded.record.kind === 1 && !linkedRetired.has(id) && finalized >= loaded.record.recoverUntil && (loaded.record.state === 4 || (loaded.record.state === 3 && loaded.record.terminal))).slice(0, Math.max(0, limit));
    for (const [id] of eligible) { try { await this.#backend.deleteStrict(id); } catch { throw new BrowserOutboxError("HOST_OUTBOX_UNAVAILABLE", "strict IndexedDB GC commit failed"); } this.#records.delete(id); } return eligible.length;
  }
  async digest(bytes: Uint8Array): Promise<Uint8Array> { return this.#crypto.digest(bytes); }
  async verifyProviderAck(publicKey: Uint8Array, message: Uint8Array, signature: Uint8Array): Promise<boolean> { return this.#crypto.verifyEd25519(publicKey, message, signature); }

  async #prepareUploadChunks(
    entry: HostOutboxEntryV1, operationCode: number, input: readonly Uint8Array[] | undefined,
  ): Promise<readonly UploadChunkRecord[]> {
    if (!input) return [];
    if (operationCode !== 1010 || Number(entry[9]) !== 0 || BigInt(entry[8]) !== 0n) {
      throw new BrowserOutboxError("HOST_OUTBOX_BINDING_INVALID", "upload spool is only valid for the initial object.put generation");
    }
    const request = decodeHostV2("RequestV2", entry[3]).value as Record<number, unknown>;
    const declared = BigInt((request[8] as Record<number, unknown>)[2] as number | bigint);
    if (input.length > 256) throw new BrowserOutboxError("HOST_OUTBOX_FULL", "upload spool chunk bound exceeded");
    const expectedCount = Number((declared + 262_143n) / 262_144n);
    if (input.length !== expectedCount) {
      throw new BrowserOutboxError("HOST_OUTBOX_BINDING_INVALID", "upload spool does not use the canonical object.put chunk count");
    }
    let length = 0n; const records: UploadChunkRecord[] = [];
    for (let index = 0; index < input.length; index += 1) {
      const exactChunk = input[index]!.slice(); const chunk = decodeHostV2("ProviderTransferChunkV1", exactChunk).value;
      const expectedLength = Number(declared - (BigInt(index) * 262_144n) >= 262_144n
        ? 262_144n
        : declared - (BigInt(index) * 262_144n));
      if (!equal(chunk[1], entry[7]) || Number(chunk[2]) !== index || !(chunk[3] instanceof Uint8Array)
        || chunk[3].length !== expectedLength || !equal(blake2b256(chunk[3]), chunk[4])) {
        throw new BrowserOutboxError("HOST_OUTBOX_BINDING_INVALID", "upload spool chunk is not exactly bound to object.put");
      }
      length += BigInt(chunk[3].length);
      const identity = concatMany([new TextEncoder().encode("cord.browser.upload-chunk.v1"), entry[1], unsigned32(index)]);
      const chunkId = (await this.#crypto.digest(identity)).slice(0, 16);
      records.push({ kind: 2, chunkId, spoolOutboxId: entry[1].slice(), operationId: entry[7].slice(), index, exactChunk, keyVersion: this.#crypto.activeKeyVersion });
    }
    if (length !== declared || (declared > 0n && records.length === 0)) {
      throw new BrowserOutboxError("HOST_OUTBOX_BINDING_INVALID", "upload spool length mismatches object.put");
    }
    return records;
  }

  #successorUploadChunk(predecessor: LiveRecord, intendedCursor: number): Uint8Array | undefined {
    const rootId = predecessor.uploadSpoolOutboxId;
    if (!rootId) throw new BrowserOutboxError("HOST_OUTBOX_STATE_INVALID", "upload successor lost its spool root");
    const root = this.#live(rootId); const chunks = root.uploadChunkIds;
    if (!chunks || root.uploadSpoolRetired) throw new BrowserOutboxError("HOST_OUTBOX_STATE_INVALID", "upload successor lost its durable chunk index");
    const index = intendedCursor - 1;
    if (index === chunks.length) return undefined;
    const chunkId = chunks[index];
    if (!chunkId) throw new BrowserOutboxError("HOST_OUTBOX_STATE_INVALID", "upload successor cursor skipped its exact durable chunk");
    return chunkId.slice();
  }

  async #commitPreparedUpload(record: LiveRecord, chunks: readonly UploadChunkRecord[]): Promise<void> {
    let rows: BrowserOutboxEncryptedRow[];
    try { rows = await Promise.all([this.#sealRecord(record), ...chunks.map((chunk) => this.#sealRecord(chunk))]); }
    catch (error) { if (error instanceof BrowserOutboxError) throw error; throw new BrowserOutboxError("HOST_OUTBOX_UNAVAILABLE", "browser upload spool encryption failed"); }
    await this.#withCommitCapacity(async () => {
      if (this.#records.size + rows.length > this.#recordLimit || this.#totalBytes() + rows.reduce((sum, row) => sum + row.ciphertext.length, 0) > this.#byteLimit) {
        throw new BrowserOutboxError("HOST_OUTBOX_FULL", "browser upload spool capacity is full");
      }
      const expected = Object.fromEntries(rows.map((row) => [row.id, null]));
      if (Object.keys(expected).length !== rows.length || rows.some((row) => this.#records.has(row.id))) {
        throw new BrowserOutboxError("HOST_OUTBOX_STATE_INVALID", "browser upload spool identity collided");
      }
      try { await this.#backend.transactStrict({ expected, puts: rows, deletes: [] }); }
      catch { throw new BrowserOutboxError("HOST_OUTBOX_UNAVAILABLE", "strict upload spool transaction failed"); }
      const records: DurableRecord[] = [record, ...chunks];
      rows.forEach((row, index) => this.#records.set(row.id, { record: records[index]!, encryptedBytes: row.ciphertext.length, encryptedRow: copyRow(row) }));
    });
  }

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
    if (!(request[1] instanceof Uint8Array) || !equal(request[1], entry[6])) throw new BrowserOutboxError("HOST_OUTBOX_BINDING_INVALID", "request ID mismatched");
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
          || Number(authority[9]) + 1 !== Number(entry[9]) || Number(authority[10]) !== Number(entry[8])
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
      this.#records.set(id, { record, encryptedBytes: row.ciphertext.length, encryptedRow: copyRow(row) });
    });
  }
  async #commitSuccessor(predecessorLoaded: LoadedRecord, predecessor: LiveRecord, successor: LiveRecord): Promise<void> {
    let predecessorRow: BrowserOutboxEncryptedRow; let successorRow: BrowserOutboxEncryptedRow;
    try { predecessorRow = await this.#sealRecord(predecessor); successorRow = await this.#sealRecord(successor); }
    catch (error) { if (error instanceof BrowserOutboxError) throw error; throw new BrowserOutboxError("HOST_OUTBOX_UNAVAILABLE", "browser outbox successor encryption failed"); }
    const predecessorId = predecessorRow.id; const successorId = successorRow.id;
    await this.#withCommitCapacity(async () => {
      if (this.#hasLiveOperationGeneration(operationGeneration(successor.entry))) throw new BrowserOutboxError("HOST_OUTBOX_STATE_INVALID", "successor operation generation became live concurrently");
      if (this.#records.size >= this.#recordLimit || this.#totalBytes() - predecessorLoaded.encryptedBytes + predecessorRow.ciphertext.length + successorRow.ciphertext.length > this.#byteLimit) {
        throw new BrowserOutboxError("HOST_OUTBOX_FULL", "browser outbox capacity is full");
      }
      try {
        await this.#backend.transactStrict({
          expected: { [predecessorId]: predecessorLoaded.encryptedRow, [successorId]: null },
          puts: [predecessorRow, successorRow], deletes: [],
        });
      } catch { throw new BrowserOutboxError("HOST_OUTBOX_UNAVAILABLE", "strict successor transaction failed"); }
      this.#records.set(predecessorId, { record: predecessor, encryptedBytes: predecessorRow.ciphertext.length, encryptedRow: copyRow(predecessorRow) });
      this.#records.set(successorId, { record: successor, encryptedBytes: successorRow.ciphertext.length, encryptedRow: copyRow(successorRow) });
    });
  }
  async #retireLinkedSuccessor(successorLoaded: LoadedRecord, tombstone: TombstoneRecord): Promise<void> {
    const successor = successorLoaded.record;
    if (successor.kind !== 0 || !successor.predecessorOutboxId) throw new BrowserOutboxError("HOST_OUTBOX_STATE_INVALID", "linked successor retirement is invalid");
    const predecessorLoaded = this.#get(successor.predecessorOutboxId); const predecessor = predecessorLoaded.record;
    if (predecessor.kind !== 0 || !predecessor.successorOutboxId || !equal(predecessor.successorOutboxId, successor.entry[1])) throw new BrowserOutboxError("HOST_OUTBOX_STATE_INVALID", "linked predecessor retirement binding mismatched");
    const { successorOutboxId: _removed, ...retainedPredecessor } = predecessor;
    const updatedPredecessor: LiveRecord = { ...retainedPredecessor, retiredSuccessorOutboxId: successor.entry[1].slice() };
    let predecessorRow: BrowserOutboxEncryptedRow; let tombstoneRow: BrowserOutboxEncryptedRow;
    try { predecessorRow = await this.#sealRecord(updatedPredecessor); tombstoneRow = await this.#sealRecord(tombstone); }
    catch (error) { if (error instanceof BrowserOutboxError) throw error; throw new BrowserOutboxError("HOST_OUTBOX_UNAVAILABLE", "linked successor retirement encryption failed"); }
    await this.#withCommitCapacity(async () => {
      if (this.#totalBytes() - predecessorLoaded.encryptedBytes - successorLoaded.encryptedBytes + predecessorRow.ciphertext.length + tombstoneRow.ciphertext.length > this.#byteLimit) throw new BrowserOutboxError("HOST_OUTBOX_FULL", "browser outbox capacity is full");
      try { await this.#backend.transactStrict({ expected: { [predecessorRow.id]: predecessorLoaded.encryptedRow, [tombstoneRow.id]: successorLoaded.encryptedRow }, puts: [predecessorRow, tombstoneRow], deletes: [] }); }
      catch { throw new BrowserOutboxError("HOST_OUTBOX_UNAVAILABLE", "strict linked successor retirement failed"); }
      this.#records.set(predecessorRow.id, { record: updatedPredecessor, encryptedBytes: predecessorRow.ciphertext.length, encryptedRow: copyRow(predecessorRow) });
      this.#records.set(tombstoneRow.id, { record: tombstone, encryptedBytes: tombstoneRow.ciphertext.length, encryptedRow: copyRow(tombstoneRow) });
    });
  }
  async #sealRecord(record: DurableRecord): Promise<BrowserOutboxEncryptedRow> {
    const idBytes = recordId(record); const id = toHex(idBytes); const plaintext = encodeRecord(record);
    const sealed = await this.#crypto.seal(idBytes, plaintext);
    if (sealed.ciphertext.length > MAX_RECORD_BYTES) throw new BrowserOutboxError("HOST_OUTBOX_FULL", "encrypted browser outbox record is too large");
    return { id, keyVersion: sealed.keyVersion, ciphertext: sealed.ciphertext.slice() };
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
  async #validateSuccessorLinks(): Promise<void> {
    const referencedChunks = new Set<string>();
    for (const loaded of this.#records.values()) {
      const record = loaded.record; if (record.kind !== 0) continue;
      if (record.predecessorOutboxId) {
        const predecessor = this.#records.get(toHex(record.predecessorOutboxId))?.record;
        if (!predecessor || predecessor.kind !== 0 || predecessor.state !== 3 || predecessor.terminal
          || !predecessor.successorOutboxId || !equal(predecessor.successorOutboxId, record.entry[1])
          || !predecessor.responseHash || !record.predecessorResponseHash || !equal(predecessor.responseHash, record.predecessorResponseHash)
          || !predecessor.successorAuthority || !equal(predecessor.successorAuthority, record.entry[4])
          || predecessor.successorCursor !== Number(record.entry[9]) || BigInt(record.entry[8]) !== BigInt(predecessor.entry[8]) + 1n
          || !equal(record.entry[7], predecessor.entry[7]) || record.operationCode !== predecessor.operationCode) throw new Error();
      }
      if (record.successorOutboxId && record.retiredSuccessorOutboxId) throw new Error();
      if (record.successorOutboxId) {
        const successor = this.#records.get(toHex(record.successorOutboxId))?.record;
        if (!successor || successor.kind !== 0 || !successor.predecessorOutboxId
          || !equal(successor.predecessorOutboxId, record.entry[1]) || !successor.predecessorResponseHash
          || !record.responseHash || !equal(successor.predecessorResponseHash, record.responseHash)) throw new Error();
      }
      if (record.retiredSuccessorOutboxId) {
        const retired = this.#records.get(toHex(record.retiredSuccessorOutboxId))?.record;
        if (!retired || retired.kind !== 1 || !equal(retired.outboxId, record.retiredSuccessorOutboxId)) throw new Error();
      }
      if (record.uploadChunkIds) {
        if (record.operationCode !== 1010 || !record.uploadSpoolOutboxId || !equal(record.uploadSpoolOutboxId, record.entry[1])) throw new Error();
        const request = decodeHostV2("RequestV2", record.entry[3]).value as Record<number, unknown>;
        const declared = BigInt((request[8] as Record<number, unknown>)[2] as number | bigint); let length = 0n;
        for (let index = 0; index < record.uploadChunkIds.length; index += 1) {
          const id = toHex(record.uploadChunkIds[index]!); const chunk = this.#records.get(id)?.record;
          if (!chunk || chunk.kind !== 2 || !equal(chunk.spoolOutboxId, record.entry[1]) || !equal(chunk.operationId, record.entry[7])
            || chunk.index !== index || referencedChunks.has(id)) throw new Error();
          const decoded = decodeHostV2("ProviderTransferChunkV1", chunk.exactChunk).value;
          if (!equal(blake2b256(decoded[3]), decoded[4])) throw new Error();
          length += BigInt(decoded[3].length); referencedChunks.add(id);
        }
        if (length !== declared) throw new Error();
      }
      if (record.uploadChunkId) {
        if (!record.uploadSpoolOutboxId || record.operationCode !== 1010 || Number(record.entry[9]) < 1) throw new Error();
        const root = this.#records.get(toHex(record.uploadSpoolOutboxId))?.record;
        const expected = root?.kind === 0 ? root.uploadChunkIds?.[Number(record.entry[9]) - 1] : undefined;
        if (!root || root.kind !== 0 || (!root.uploadSpoolRetired && (!expected || !equal(expected, record.uploadChunkId)))) throw new Error();
      } else if (record.uploadSpoolOutboxId && record.predecessorOutboxId) {
        const root = this.#records.get(toHex(record.uploadSpoolOutboxId))?.record;
        if (!root || root.kind !== 0 || (!root.uploadSpoolRetired && Number(record.entry[9]) - 1 !== root.uploadChunkIds?.length)) throw new Error();
      }
    }
    for (const [id, loaded] of this.#records) if (loaded.record.kind === 2 && !referencedChunks.has(id)) throw new Error();
  }
  #totalBytes(): number { let total = 0; for (const loaded of this.#records.values()) total += loaded.encryptedBytes; return total; }
}

export function providerAckConfirmationMessage(context: BrowserHostOutboxContextV1, outboxId: Uint8Array, responseHash: Uint8Array): Uint8Array {
  return encodeHostV2Value({ 0: 1, 1: outboxId, 2: responseHash, 3: context.providerId, 4: context.providerEndpointHash, 5: context.negotiatedTuple });
}
function retry(record: LiveRecord, records: ReadonlyMap<string, LoadedRecord>): BrowserOutboxRetryV1 {
  const upload = record.uploadChunkId ? records.get(toHex(record.uploadChunkId))?.record : undefined;
  if (record.uploadChunkId && (!upload || upload.kind !== 2 || !record.uploadSpoolOutboxId
    || !equal(upload.spoolOutboxId, record.uploadSpoolOutboxId) || !equal(upload.operationId, record.entry[7]))) {
    throw new BrowserOutboxError("HOST_OUTBOX_STATE_INVALID", "durable upload chunk is absent or misbound");
  }
  return {
    request: record.entry[3].slice(), authority: record.entry[4].slice(), requestId: record.entry[6].slice(), operationId: record.entry[7].slice(),
    outboxId: record.entry[1].slice(), fingerprint: record.entry[5].slice(), expectedResponseKind: Number(record.entry[15]),
    intendedCursor: Number(record.entry[9]), cancel: isCancel(record.entry[3]), operationCode: record.operationCode,
    ...(upload?.kind === 2 ? { uploadChunk: upload.exactChunk.slice() } : {}),
  };
}
function isCancel(bytes: Uint8Array): boolean { try { decodeHostV2("CancelledEventV2", bytes); return true; } catch { return false; } }
function cloneEntry(entry: HostOutboxEntryV1): HostOutboxEntryV1 { return decodeHostV2("HostOutboxEntryV1", encodeHostV2("HostOutboxEntryV1", entry)).value; }
function recordId(record: DurableRecord): Uint8Array { return record.kind === 0 ? record.entry[1] : record.kind === 1 ? record.outboxId : record.chunkId; }
function recordKeyVersion(record: DurableRecord): number { return record.kind === 0 ? Number(record.entry[20]) : record.keyVersion; }
function encodeRecord(record: DurableRecord): Uint8Array {
  const map: HostV2Map = record.kind === 0 ? {
    0: 1, 1: 0, 2: encodeHostV2("HostOutboxEntryV1", record.entry), 3: record.state, 7: record.terminal, 8: record.recoverUntil, 12: record.operationCode,
    ...(record.response ? { 4: record.response } : {}), ...(record.responseAck ? { 5: record.responseAck } : {}), ...(record.responseHash ? { 6: record.responseHash } : {}),
    ...(record.successorAuthority ? { 13: record.successorAuthority } : {}), ...(record.successorCursor !== undefined ? { 14: record.successorCursor } : {}),
    ...(record.predecessorOutboxId ? { 15: record.predecessorOutboxId } : {}), ...(record.predecessorResponseHash ? { 16: record.predecessorResponseHash } : {}),
    ...(record.successorOutboxId ? { 17: record.successorOutboxId } : {}), ...(record.retiredSuccessorOutboxId ? { 18: record.retiredSuccessorOutboxId } : {}),
    ...(record.uploadSpoolOutboxId ? { 19: record.uploadSpoolOutboxId } : {}), ...(record.uploadChunkIds ? { 20: record.uploadChunkIds } : {}),
    ...(record.uploadChunkId ? { 21: record.uploadChunkId } : {}), ...(record.uploadSpoolRetired ? { 22: true } : {}),
  } : record.kind === 1
    ? { 0: 1, 1: 1, 3: record.state, 7: record.terminal, 8: record.recoverUntil, 9: record.outboxId, 10: record.keyVersion, 11: record.requestFingerprint, ...(record.responseHash ? { 6: record.responseHash } : {}), ...(record.uploadSpoolOutboxId ? { 19: record.uploadSpoolOutboxId } : {}), ...(record.uploadSpoolRetired ? { 22: true } : {}) }
    : { 0: 1, 1: 2, 9: record.chunkId, 10: record.keyVersion, 19: record.spoolOutboxId, 20: record.operationId, 21: record.index, 22: record.exactChunk };
  return encodeHostV2Value(map);
}
function decodeRecord(bytes: Uint8Array): DurableRecord {
  const value = decodeCanonicalHostV2Value(bytes); if (typeof value !== "object" || value === null || Array.isArray(value) || value instanceof Uint8Array) throw new Error(); const map = value as HostV2Map;
  const keys = Object.keys(map).map(Number).sort((a, b) => a - b);
  if (map[0] !== 1 || ![0, 1, 2].includes(Number(map[1]))) throw new Error();
  if (map[1] === 2) {
    if (keys.some((key) => ![0, 1, 9, 10, 19, 20, 21, 22].includes(key)) || ![0, 1, 9, 10, 19, 20, 21, 22].every((key) => keys.includes(key))
      || !(map[9] instanceof Uint8Array) || map[9].length !== 16 || typeof map[10] !== "number"
      || !(map[19] instanceof Uint8Array) || map[19].length !== 16 || !(map[20] instanceof Uint8Array) || map[20].length !== 16
      || typeof map[21] !== "number" || !Number.isSafeInteger(map[21]) || map[21] < 0 || map[21] >= 256 || !(map[22] instanceof Uint8Array)) throw new Error();
    const exactChunk = map[22].slice(); const chunk = decodeHostV2("ProviderTransferChunkV1", exactChunk).value;
    if (!equal(chunk[1], map[20]) || Number(chunk[2]) !== map[21] || !(chunk[3] instanceof Uint8Array) || chunk[3].length > 262_144) throw new Error();
    return { kind: 2, chunkId: map[9].slice(), keyVersion: map[10], spoolOutboxId: map[19].slice(), operationId: map[20].slice(), index: map[21], exactChunk };
  }
  if (typeof map[3] !== "number" || typeof map[7] !== "boolean" || (typeof map[8] !== "number" && typeof map[8] !== "bigint")) throw new Error();
  if (map[1] === 0) {
    if (keys.some((key) => ![0, 1, 2, 3, 4, 5, 6, 7, 8, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22].includes(key)) || ![0, 1, 2, 3, 7, 8, 12].every((key) => keys.includes(key)) || !(map[2] instanceof Uint8Array) || ![0, 1, 2, 3].includes(map[3]) || typeof map[12] !== "number") throw new Error();
    const state = map[3] as 0 | 1 | 2 | 3; const response = optionalBytes(map[4]); const ack = optionalBytes(map[5]); const hash = optionalBytes(map[6], 32);
    const successorAuthority = optionalBytes(map[13]); const rawSuccessorCursor = map[14];
    if (rawSuccessorCursor !== undefined && (typeof rawSuccessorCursor !== "number" || !Number.isSafeInteger(rawSuccessorCursor) || rawSuccessorCursor < 0)) throw new Error();
    const successorCursor = rawSuccessorCursor as number | undefined;
    const predecessorOutboxId = optionalBytes(map[15], 16); const predecessorResponseHash = optionalBytes(map[16], 32); const successorOutboxId = optionalBytes(map[17], 16); const retiredSuccessorOutboxId = optionalBytes(map[18], 16);
    const uploadSpoolOutboxId = optionalBytes(map[19], 16); const uploadChunkId = optionalBytes(map[21], 16);
    const uploadChunkIds = map[20] === undefined ? undefined : Array.isArray(map[20]) ? map[20].map((id) => optionalBytes(id, 16)!) : (() => { throw new Error(); })();
    if (map[22] !== undefined && map[22] !== true) throw new Error(); const uploadSpoolRetired = map[22] === true;
    const hasResponse = Boolean(response && ack && hash); const hasSuccessor = Boolean(successorAuthority) && successorCursor !== undefined;
    if ((state >= 2) !== hasResponse || Boolean(successorAuthority) !== (successorCursor !== undefined)
      || Boolean(predecessorOutboxId) !== Boolean(predecessorResponseHash) || (state < 2 && hasSuccessor)
      || (state === 2 && map[7] === false && !hasSuccessor) || (state === 3 && (map[7] !== false || !hasSuccessor))
      || (map[7] === true && state !== 2) || (successorOutboxId && state !== 3) || (retiredSuccessorOutboxId && state !== 3)
      || Boolean(successorOutboxId) && Boolean(retiredSuccessorOutboxId)) throw new Error();
    if (Boolean(uploadChunkIds) && (!uploadSpoolOutboxId || uploadChunkId || uploadSpoolRetired || !equal(uploadSpoolOutboxId, (decodeHostV2("HostOutboxEntryV1", map[2]).value as HostOutboxEntryV1)[1]))
      || Boolean(uploadChunkId) && (!uploadSpoolOutboxId || uploadChunkIds || uploadSpoolRetired)
      || uploadSpoolRetired && (!uploadSpoolOutboxId || uploadChunkId || uploadChunkIds)) throw new Error();
    return {
      kind: 0, entry: decodeHostV2("HostOutboxEntryV1", map[2]).value, state,
      ...(response ? { response } : {}), ...(ack ? { responseAck: ack } : {}), ...(hash ? { responseHash: hash } : {}),
      ...(successorAuthority ? { successorAuthority } : {}), ...(successorCursor !== undefined ? { successorCursor } : {}),
      ...(predecessorOutboxId ? { predecessorOutboxId } : {}), ...(predecessorResponseHash ? { predecessorResponseHash } : {}),
      ...(successorOutboxId ? { successorOutboxId } : {}), ...(retiredSuccessorOutboxId ? { retiredSuccessorOutboxId } : {}), terminal: map[7], recoverUntil: BigInt(map[8] as number | bigint), operationCode: map[12],
      ...(uploadSpoolOutboxId ? { uploadSpoolOutboxId } : {}), ...(uploadChunkIds ? { uploadChunkIds } : {}),
      ...(uploadChunkId ? { uploadChunkId } : {}), ...(uploadSpoolRetired ? { uploadSpoolRetired: true } : {}),
    };
  }
  if (keys.some((key) => ![0, 1, 3, 6, 7, 8, 9, 10, 11, 19, 22].includes(key)) || ![0, 1, 3, 7, 8, 9, 10, 11].every((key) => keys.includes(key)) || ![3, 4].includes(map[3]) || !(map[9] instanceof Uint8Array) || map[9].length !== 16 || typeof map[10] !== "number" || !(map[11] instanceof Uint8Array) || map[11].length !== 32) throw new Error();
  const responseHash = optionalBytes(map[6], 32);
  const uploadSpoolOutboxId = optionalBytes(map[19], 16); if (map[22] !== undefined && map[22] !== true) throw new Error();
  const uploadSpoolRetired = map[22] === true; if (uploadSpoolRetired && !uploadSpoolOutboxId) throw new Error();
  if (map[3] === 3 && (!responseHash || map[7] !== true)) throw new Error();
  return { kind: 1, outboxId: map[9].slice(), state: map[3] as 3 | 4, ...(responseHash ? { responseHash } : {}), requestFingerprint: map[11].slice(), terminal: map[7], recoverUntil: BigInt(map[8] as number | bigint), keyVersion: map[10], ...(uploadSpoolOutboxId ? { uploadSpoolOutboxId } : {}), ...(uploadSpoolRetired ? { uploadSpoolRetired: true } : {}) };
}
function optionalBytes(value: unknown, length?: number): Uint8Array | undefined { if (value === undefined) return undefined; if (!(value instanceof Uint8Array) || (length !== undefined && value.length !== length)) throw new Error(); return value.slice(); }
function equal(left: Uint8Array, right: Uint8Array): boolean { return left.length === right.length && left.every((byte, index) => byte === right[index]); }
function toHex(bytes: Uint8Array): string { return [...bytes].map((byte) => byte.toString(16).padStart(2, "0")).join(""); }
function fromHex(value: string): Uint8Array { return Uint8Array.from(value.match(/../g)!.map((byte) => Number.parseInt(byte, 16))); }
function operationGeneration(entry: HostOutboxEntryV1): string { return `${toHex(entry[7])}:${BigInt(entry[8]).toString(10)}`; }
function operationRequiresId(operationCode: number): boolean { return Object.values(HOST_V2_OPERATION_BINDINGS).some(({ code, operationIdRequired }) => code === operationCode && operationIdRequired); }
function authorityHostKey(authority: Uint8Array): Uint8Array { try { return decodeHostV2("ProviderCapabilityV1", authority).value[4]; } catch { return decodeHostV2("ResumeTokenV1", authority).value[4]; } }
function concatMany(parts: readonly Uint8Array[]): Uint8Array { const output = new Uint8Array(parts.reduce((sum, part) => sum + part.length, 0)); let offset = 0; for (const part of parts) { output.set(part, offset); offset += part.length; } return output; }
function unsigned32(value: number): Uint8Array { return Uint8Array.of(value >>> 24, value >>> 16, value >>> 8, value); }
