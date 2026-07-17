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

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";
import { parseContentCid } from "../../origin-sdk-cloud-storage/src/content.ts";
import { blake2b256 } from "../../origin-sdk-crypto/src/index.ts";
import { DurableBrowserHostV2 } from "../src/internal/v2/browser-durable.ts";
import {
  BrowserHostOutboxV1, BrowserOutboxError, providerAckConfirmationMessage,
  type BrowserOutboxEncryptedRow, type StrictBrowserOutboxBackend, type StrictBrowserOutboxTransactionV1,
} from "../src/internal/v2/browser-outbox.ts";
import { BrowserHostOutboxKeyRingV1, BrowserXChaCha20Poly1305, type BrowserHostOutboxContextV1 } from "../src/internal/v2/browser-crypto.ts";
import { BROWSER_HOST_V2_WINDOW, BrowserHostV2Transport, BrowserHostV2TransportError, type BrowserHostV2Peer } from "../src/internal/v2/browser.ts";
import { decodeCanonicalHostV2Value, decodeHostV2, encodeHostV2, encodeHostV2Value } from "../src/internal/v2/codec.ts";
import {
  HOST_V2_FEATURE_IDS, HOST_V2_MAJOR, HOST_V2_MINOR, HOST_V2_OPERATION_BINDINGS, HOST_V2_PROTOCOL, HOST_V2_REGISTRY_SHA256,
  type AcceptedEventV2, type CancelledEventV2, type HostOutboxEntryV1, type ProgressEventV2,
} from "../src/internal/v2/generated.ts";
import type { HostV2NegotiationOffer } from "../src/internal/v2/session.ts";
import {
  PrivateDurableBrowserHostV2, PrivateDurableBrowserStorageV2, PrivateOriginBrowserRouterV2, runPrivateBrowserRustProviderV2,
  type PrivateBrowserRustProviderBridgeV2, type PrivateFinalizedHostAuthorityV2,
} from "../../../internal/browser-host-v2.ts";
import { PrivateCordCommonsRuntimeBridgeV2 } from "../../../internal/commons-runtime-bridge-v2.ts";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "../../../..");
const frozen = JSON.parse(readFileSync(resolve(root, "docs/specs/origin-host-registry-v2.vectors.json"), "utf8"));
const protocol = JSON.parse(readFileSync(resolve(root, "docs/specs/protocol-executable-v2.vectors.json"), "utf8"));
const transferChunks = JSON.parse(readFileSync(resolve(root, "docs/specs/provider-transfer-chunk-v1.vectors.json"), "utf8"));
const bytes = (hex: string): Uint8Array => Uint8Array.from(Buffer.from(hex, "hex"));
const equal = (left: Uint8Array, right: Uint8Array): boolean => left.length === right.length && left.every((byte, index) => byte === right[index]);

function offer(): HostV2NegotiationOffer {
  return { protocol: HOST_V2_PROTOCOL, major: HOST_V2_MAJOR, minors: [HOST_V2_MINOR], genesis: new Uint8Array(32).fill(0x42), finalizedSpecVersion: 31, finalizedTransactionVersion: 8, registrySha256: HOST_V2_REGISTRY_SHA256, features: [...HOST_V2_FEATURE_IDS] };
}
interface PeerKey { readonly peer: BrowserHostV2Peer; readonly privateKey: CryptoKey }
async function peer(source: string): Promise<PeerKey> {
  const pair = await globalThis.crypto.subtle.generateKey("Ed25519", true, ["sign", "verify"]);
  const publicKey = new Uint8Array(await globalThis.crypto.subtle.exportKey("raw", pair.publicKey));
  const providerId = new Uint8Array(await globalThis.crypto.subtle.digest("SHA-256", publicKey));
  const endpointHash = new Uint8Array(await globalThis.crypto.subtle.digest("SHA-256", new TextEncoder().encode(source)));
  return { peer: { source, channel: "festival-v2", providerId, endpointHash, acknowledgementPublicKey: publicKey }, privateKey: pair.privateKey };
}
const negotiationSigner = (key: PeerKey) => async (message: Uint8Array): Promise<Uint8Array> => new Uint8Array(await globalThis.crypto.subtle.sign("Ed25519", key.privateKey, Uint8Array.from(message).buffer));
function concatenate(parts: readonly Uint8Array[]): Uint8Array { const output = new Uint8Array(parts.reduce((length, part) => length + part.length, 0)); let offset = 0; for (const part of parts) { output.set(part, offset); offset += part.length; } return output; }
function unsigned(value: number, length: number): Uint8Array { const output = new Uint8Array(length); for (let index = length - 1, remaining = value; index >= 0; index -= 1) { output[index] = remaining & 0xff; remaining = Math.floor(remaining / 256); } return output; }
function text(value: string): Uint8Array { const encoded = new TextEncoder().encode(value); return concatenate([unsigned(encoded.length, 2), encoded]); }
function offerMessage(remote: BrowserHostV2Peer, value: HostV2NegotiationOffer): Uint8Array {
  return concatenate([
    new TextEncoder().encode("cord.origin.host/2/browser-offer/v1"), text(remote.source), text(remote.channel), remote.providerId, remote.endpointHash,
    remote.acknowledgementPublicKey, text(value.protocol), Uint8Array.of(value.major), unsigned(value.minors.length, 2), ...value.minors.map((minor) => unsigned(minor, 2)),
    value.genesis, unsigned(value.finalizedSpecVersion, 4), unsigned(value.finalizedTransactionVersion, 4), text(value.registrySha256),
    unsigned(value.features.length, 2), ...value.features.map(text),
  ]);
}
function acknowledgementMessage(remote: BrowserHostV2Peer, digest: Uint8Array): Uint8Array {
  return concatenate([new TextEncoder().encode("cord.origin.host/2/browser-negotiated/v1"), text(remote.source), text(remote.channel), remote.providerId, remote.endpointHash, digest]);
}
async function tupleDigest(value: HostV2NegotiationOffer): Promise<Uint8Array> {
  const features = value.features.map((feature) => new TextEncoder().encode(feature));
  const registry = bytes(value.registrySha256);
  return new Uint8Array(await globalThis.crypto.subtle.digest("SHA-256", concatenate([
    new TextEncoder().encode("cord.origin.host/2/negotiated-tuple/v1"), Uint8Array.of(value.major), unsigned(Math.max(...value.minors), 2), value.genesis,
    unsigned(value.finalizedSpecVersion, 4), unsigned(value.finalizedTransactionVersion, 4), registry, unsigned(features.length, 2),
    ...features.flatMap((feature) => [unsigned(feature.length, 2), feature]),
  ])));
}
async function signedEnvelope(key: PeerKey, local: BrowserHostV2Peer, kind: "offer" | "negotiated", digest?: Uint8Array): Promise<Record<string, unknown>> {
  const value = offer();
  if (kind === "offer") return {
    version: 2, channel: key.peer.channel, source: key.peer.source, target: local.source, kind,
    providerId: key.peer.providerId, endpointHash: key.peer.endpointHash, acknowledgementPublicKey: key.peer.acknowledgementPublicKey,
    offer: { protocol: value.protocol, major: value.major, minors: value.minors, genesis: value.genesis, finalizedSpecVersion: value.finalizedSpecVersion, finalizedTransactionVersion: value.finalizedTransactionVersion, registrySha256: value.registrySha256, features: value.features },
    signature: await negotiationSigner(key)(offerMessage(key.peer, value)),
  };
  const exactDigest = digest ?? await tupleDigest(value);
  return { version: 2, channel: key.peer.channel, source: key.peer.source, target: local.source, kind, digest: exactDigest, signature: await negotiationSigner(key)(acknowledgementMessage(key.peer, exactDigest)) };
}
async function transports(hostPeer?: PeerKey, providerPeer?: PeerKey): Promise<{ host: BrowserHostV2Transport; provider: BrowserHostV2Transport; hostPeer: PeerKey; providerPeer: PeerKey }> {
  const hostKey = hostPeer ?? await peer("host"); const providerKey = providerPeer ?? await peer("provider"); const channel = new MessageChannel();
  const [host, provider] = await Promise.all([
    BrowserHostV2Transport.connect(channel.port1, hostKey.peer, providerKey.peer, (bound, port) => { assert.ok(equal(bound.providerId, providerKey.peer.providerId)); assert.equal(port, channel.port1); }, offer(), negotiationSigner(hostKey), { timeoutMs: 1_000 }),
    BrowserHostV2Transport.connect(channel.port2, providerKey.peer, hostKey.peer, (bound, port) => { assert.ok(equal(bound.providerId, hostKey.peer.providerId)); assert.equal(port, channel.port2); }, offer(), negotiationSigner(providerKey), { timeoutMs: 1_000 }),
  ]);
  return { host, provider, hostPeer: hostKey, providerPeer: providerKey };
}
class StrictMemoryBackend implements StrictBrowserOutboxBackend {
  readonly records = new Map<string, BrowserOutboxEncryptedRow>(); readonly quarantine = new Map<string, BrowserOutboxEncryptedRow>();
  failBeforeCommit = false; failAfterCommit = false; failTransactionBeforeCommit = false; strictCommits = 0; putDelayMs = 0; putStarts = 0;
  async load() { return [...this.records.values()].map(copyRow); }
  async putStrict(row: BrowserOutboxEncryptedRow) { this.putStarts += 1; if (this.putDelayMs > 0) await new Promise((resolve) => setTimeout(resolve, this.putDelayMs)); if (this.failBeforeCommit) { this.failBeforeCommit = false; throw new Error("abort"); } this.records.set(row.id, copyRow(row)); this.strictCommits += 1; if (this.failAfterCommit) { this.failAfterCommit = false; throw new Error("after commit"); } }
  async deleteStrict(id: string) { this.records.delete(id); this.strictCommits += 1; }
  async quarantineStrict(row: BrowserOutboxEncryptedRow) { this.quarantine.set(row.id, copyRow(row)); this.records.delete(row.id); this.strictCommits += 1; }
  async transactStrict(input: StrictBrowserOutboxTransactionV1) {
    for (const [id, expected] of Object.entries(input.expected)) {
      const current = this.records.get(id);
      const same = expected === null ? current === undefined : current !== undefined && current.keyVersion === expected.keyVersion && equal(current.ciphertext, expected.ciphertext);
      if (!same) throw new Error("compare-and-swap failed");
    }
    if (this.failTransactionBeforeCommit) { this.failTransactionBeforeCommit = false; throw new Error("transaction abort"); }
    for (const row of input.puts) this.records.set(row.id, copyRow(row));
    for (const id of input.deletes) this.records.delete(id);
    this.strictCommits += 1;
  }
}
function copyRow(row: BrowserOutboxEncryptedRow): BrowserOutboxEncryptedRow { return { id: row.id, keyVersion: row.keyVersion, ciphertext: row.ciphertext.slice() }; }
function nonceSource() { let counter = 0; return (length: number): Uint8Array => { const value = new Uint8Array(length); value.fill(0x80); value[length - 1] = counter++; return value; }; }
function keyring(active = 1, includeOld = true): BrowserHostOutboxKeyRingV1 { return new BrowserHostOutboxKeyRingV1(active, new Map([...(includeOld ? [[1, new Uint8Array(32).fill(0x8a)] as const] : []), ...(active === 2 ? [[2, new Uint8Array(32).fill(0x9a)] as const] : [])])); }
function context(transport: BrowserHostV2Transport): BrowserHostOutboxContextV1 { const binding = transport.binding; return { profileId: new Uint8Array(32).fill(0x77), registryHash: binding.registryHash, genesisHash: binding.genesisHash, negotiatedTuple: binding.negotiatedTuple, providerId: binding.providerId, providerEndpointHash: binding.providerEndpointHash }; }
async function preparedEntry(transport: BrowserHostV2Transport, version = 1, options: { readonly vector?: string; readonly outbox?: number; readonly request?: number; readonly operation?: number; readonly expected?: number } = {}): Promise<HostOutboxEntryV1> {
  const requestVector = frozen.vectors.find((vector: any) => vector.id === (options.vector ?? "1010-positive"));
  const request = decodeHostV2("RequestV2", bytes(requestVector.wire_hex)).value as any;
  const requestId = new Uint8Array(16).fill(options.request ?? 0x55); const operationId = new Uint8Array(16).fill(options.operation ?? 0x44); const outboxId = new Uint8Array(16).fill(options.outbox ?? 0x66); const grantId = new Uint8Array(32).fill(0x33);
  request[1] = requestId; if (request[4] !== undefined) request[4] = grantId; if (request[5] !== undefined) request[5] = operationId; request[7] = 0;
  const requestBytes = encodeHostV2("RequestV2", request);
  const authorityVector = protocol.vectors.find((vector: any) => vector.id === "provider-capability-v1");
  const authority = decodeHostV2("ProviderCapabilityV1", bytes(authorityVector.canonical_cbor_hex)).value as any;
  const binding = transport.binding; authority[1] = binding.registryHash; authority[2] = binding.genesisHash; authority[3] = grantId; authority[8] = binding.providerId; authority[9] = [request[3]];
  const authorityBytes = encodeHostV2("ProviderCapabilityV1", authority);
  const joined = new Uint8Array(requestBytes.length + authorityBytes.length); joined.set(requestBytes); joined.set(authorityBytes, requestBytes.length);
  const fingerprint = new Uint8Array(await globalThis.crypto.subtle.digest("SHA-256", joined));
  return { 0: 1, 1: outboxId, 2: 0, 3: requestBytes, 4: authorityBytes, 5: fingerprint, 6: requestId, 7: request[5] === undefined ? new Uint8Array(16) : operationId, 8: 0, 9: 0, 10: binding.registryHash, 11: binding.genesisHash, 12: binding.negotiatedTuple, 13: binding.providerId, 14: binding.providerEndpointHash, 15: options.expected ?? 4, 17: 100, 18: 228, 19: 356, 20: version };
}
function exactResumeToken(transport: BrowserHostV2Transport, entry: HostOutboxEntryV1, cursor: number, cancelled = false): Uint8Array {
  const vector = protocol.vectors.find((candidate: any) => candidate.id === "provider-resume-v1");
  const token = decodeHostV2("ResumeTokenV1", bytes(vector.canonical_cbor_hex)).value as any; const binding = transport.binding;
  token[1] = binding.registryHash; token[2] = binding.genesisHash; token[3] = binding.providerId; token[5] = entry[7];
  token[4] = (decodeHostV2("ProviderCapabilityV1", entry[4]).value as any)[4]; token[6] = (decodeHostV2("RequestV2", entry[3]).value as any)[8][0];
  token[7] = (decodeHostV2("RequestV2", entry[3]).value as any)[8][1]; token[8] = (decodeHostV2("RequestV2", entry[3]).value as any)[8][2];
  token[9] = cursor - 1; token[10] = Number(entry[8]) + 1; token[11] = 100; token[12] = 228; token[14] = cancelled;
  return encodeHostV2("ResumeTokenV1", token);
}
function exactCapability(transport: BrowserHostV2Transport, frame: any, length?: number): Uint8Array {
  const vector = protocol.vectors.find((candidate: any) => candidate.id === "provider-capability-v1");
  const capability = decodeHostV2("ProviderCapabilityV1", bytes(vector.canonical_cbor_hex)).value as any;
  capability[1] = transport.binding.registryHash; capability[2] = transport.binding.genesisHash; capability[3] = frame[4];
  capability[5] = frame[2]; capability[6] = frame[8][0]; capability[8] = transport.binding.providerId; capability[9] = [frame[3]];
  if (length !== undefined) capability[11] = length; capability[12] = 100; capability[13] = 228;
  return encodeHostV2("ProviderCapabilityV1", capability);
}
async function signedResumeToken(
  transport: BrowserHostV2Transport, signer: CryptoKey, frame: any, exactAuthority: Uint8Array, intendedCursor: number, generation = 1, cancelled = false,
): Promise<Uint8Array> {
  const vector = protocol.vectors.find((candidate: any) => candidate.id === "provider-resume-v1");
  const token = decodeHostV2("ResumeTokenV1", bytes(vector.canonical_cbor_hex)).value as any;
  token[1] = transport.binding.registryHash; token[2] = transport.binding.genesisHash; token[3] = transport.binding.providerId;
  token[4] = (decodeHostV2("ProviderCapabilityV1", exactAuthority).value as any)[4]; token[5] = frame[5]; token[6] = frame[8][0];
  token[7] = frame[8][1]; token[8] = frame[8][2]; token[9] = intendedCursor - 1; token[10] = generation;
  token[11] = 100; token[12] = 228; token[14] = cancelled;
  const signed = encodeHostV2Value(Object.fromEntries(Array.from({ length: 15 }, (_, index) => [index, token[index]])) as any);
  token[15] = new Uint8Array(await globalThis.crypto.subtle.sign("Ed25519", signer, concatenate([new TextEncoder().encode("cord.provider.resume.v1"), signed])));
  return encodeHostV2("ResumeTokenV1", token);
}
async function successorEntry(predecessor: HostOutboxEntryV1, token: Uint8Array, cursor: number, outbox = 0x67): Promise<HostOutboxEntryV1> {
  const joined = concatenate([predecessor[3], token]); const fingerprint = new Uint8Array(await globalThis.crypto.subtle.digest("SHA-256", joined));
  return { ...predecessor, 1: new Uint8Array(16).fill(outbox), 2: 0, 4: token.slice(), 5: fingerprint, 8: BigInt(predecessor[8]) + 1n, 9: cursor, 17: 100, 18: 228, 19: 356 };
}
function accepted(id: Uint8Array, sequence = 0) { return encodeHostV2("AcceptedEventV2", { 0: 2, 1: id, 2: sequence, 3: 0, 4: { 0: 0 } } as AcceptedEventV2); }
function progress(id: Uint8Array, sequence: number) { return encodeHostV2("ProgressEventV2", { 0: 2, 1: id, 2: sequence, 3: 1, 4: { 0: 1 } } as ProgressEventV2); }
function cancelled(id: Uint8Array, sequence: number) { return encodeHostV2("CancelledEventV2", { 0: 2, 1: id, 2: sequence, 3: 4, 4: { 0: 107 } } as CancelledEventV2); }
async function openOutbox(backend: StrictMemoryBackend, transport: BrowserHostV2Transport, keys = keyring(), random = nonceSource()) { return BrowserHostOutboxV1.open(backend, context(transport), keys, { random }); }

async function signConfirmation(privateKey: CryptoKey, outbox: BrowserHostOutboxV1, outboxId: Uint8Array, responseHash: Uint8Array) {
  const message = providerAckConfirmationMessage(outbox.contextBinding, outboxId, responseHash);
  return new Uint8Array(await globalThis.crypto.subtle.sign("Ed25519", privateKey, message));
}

test("strict browser backend transaction atomically compares, replaces, and creates records", async () => {
  const backend = new StrictMemoryBackend(); const first = { id: "first", keyVersion: 1, ciphertext: Uint8Array.of(1) };
  await backend.putStrict(first);
  await backend.transactStrict({
    expected: { first, second: null },
    puts: [{ id: "first", keyVersion: 1, ciphertext: Uint8Array.of(2) }, { id: "second", keyVersion: 1, ciphertext: Uint8Array.of(3) }], deletes: [],
  });
  assert.deepEqual(backend.records.get("first")?.ciphertext, Uint8Array.of(2)); assert.ok(backend.records.has("second"));
  await assert.rejects(backend.transactStrict({ expected: { first }, puts: [], deletes: ["second"] }), /compare-and-swap/);
  assert.ok(backend.records.has("second"), "failed CAS partially deleted a record");
});

test("browser byte plane consumes the exact Rust-shared BLAKE2b-256 transfer vector", () => {
  assert.equal(transferChunks.schema_version, 1); assert.equal(transferChunks.algorithm, "BLAKE2b-256");
  const vector = transferChunks.vectors.find((candidate: any) => candidate.id === "cord-byte-plane-v1");
  const payload = bytes(vector.bytes_hex); const digest = blake2b256(payload); assert.equal(Buffer.from(digest).toString("hex"), vector.digest_hex);
  const exact = encodeHostV2("ProviderTransferChunkV1", { 0: 1, 1: bytes(vector.operation_id_hex), 2: vector.index, 3: payload, 4: digest });
  assert.equal(Buffer.from(exact).toString("hex"), vector.canonical_cbor_hex);
  assert.deepEqual(encodeHostV2("ProviderTransferChunkV1", decodeHostV2("ProviderTransferChunkV1", exact).value), exact);
});

test("acknowledged resume successor survives crashes and advances generation and cursor exactly once", async () => {
  const pair = await transports(); const backend = new StrictMemoryBackend(); const random = nonceSource();
  let outbox = await openOutbox(backend, pair.host, keyring(), random); const entry = await preparedEntry(pair.host);
  const prepared = await outbox.prepare({ entry }); await outbox.markSent(prepared.outboxId);
  const event = progress(entry[6], 1); const token = exactResumeToken(pair.host, entry, 2);
  const installed = await outbox.installSuccessor(prepared.outboxId, event, token, 2);

  outbox = await openOutbox(backend, pair.host, keyring(), random);
  assert.deepEqual(await outbox.installSuccessor(prepared.outboxId, event, token, 2), installed, "crash replay changed the installed successor");
  await outbox.confirmAck(prepared.outboxId, installed.responseHash);
  assert.throws(() => outbox.retry(prepared.outboxId, 200n), /not retryable/);

  const successor = await successorEntry(entry, token, 2); const resumed = await outbox.prepareSuccessor(prepared.outboxId, { entry: successor });
  assert.equal(resumed.intendedCursor, 2); assert.equal(resumed.operationCode, 1010); assert.deepEqual(resumed.authority, token);
  assert.deepEqual(await outbox.prepareSuccessor(prepared.outboxId, { entry: successor }), resumed, "exact replay was not idempotent");
  outbox = await openOutbox(backend, pair.host, keyring(), random);
  assert.deepEqual(outbox.retry(resumed.outboxId, 200n), resumed); assert.equal(backend.records.size, 2, "acknowledged predecessor was discarded");
  assert.deepEqual(outbox.linkedSuccessor(prepared.outboxId, 200n), resumed, "post-commit restart lost its exact linked successor handle");

  const changed = await successorEntry(entry, token, 2, 0x68);
  await assert.rejects(outbox.prepareSuccessor(prepared.outboxId, { entry: changed }), (error) => error instanceof BrowserOutboxError && error.code === "HOST_OUTBOX_STATE_INVALID");
  pair.host.close(); pair.provider.close();
});

test("successor transaction abort leaves predecessor replayable and creates no partial successor", async () => {
  const pair = await transports(); const backend = new StrictMemoryBackend(); const random = nonceSource();
  let outbox = await openOutbox(backend, pair.host, keyring(), random); const entry = await preparedEntry(pair.host);
  const prepared = await outbox.prepare({ entry }); const event = progress(entry[6], 1); const token = exactResumeToken(pair.host, entry, 2);
  const installed = await outbox.installSuccessor(prepared.outboxId, event, token, 2); await outbox.confirmAck(prepared.outboxId, installed.responseHash);
  const successor = await successorEntry(entry, token, 2); backend.failTransactionBeforeCommit = true;
  await assert.rejects(outbox.prepareSuccessor(prepared.outboxId, { entry: successor }), (error) => error instanceof BrowserOutboxError && error.code === "HOST_OUTBOX_UNAVAILABLE");
  assert.equal(backend.records.size, 1, "failed successor transaction partially committed");
  outbox = await openOutbox(backend, pair.host, keyring(), random);
  assert.deepEqual((await outbox.prepareSuccessor(prepared.outboxId, { entry: successor })).authority, token); assert.equal(backend.records.size, 2);
  pair.host.close(); pair.provider.close();
});

test("terminal successor acknowledgement atomically unlinks its predecessor and reopens without corruption", async () => {
  const pair = await transports(); const backend = new StrictMemoryBackend(); const random = nonceSource(); let outbox = await openOutbox(backend, pair.host, keyring(), random);
  const entry = await preparedEntry(pair.host); const prepared = await outbox.prepare({ entry }); const token = exactResumeToken(pair.host, entry, 2);
  const installed = await outbox.installSuccessor(prepared.outboxId, progress(entry[6], 1), token, 2); await outbox.confirmAck(prepared.outboxId, installed.responseHash);
  const successor = await outbox.prepareSuccessor(prepared.outboxId, { entry: await successorEntry(entry, token, 2) });
  const terminal = await outbox.installTerminal(successor.outboxId, cancelled(entry[6], 2), 400n); await outbox.confirmAck(successor.outboxId, terminal.responseHash);
  outbox = await openOutbox(backend, pair.host, keyring(), random); assert.throws(() => outbox.retry(successor.outboxId, 0n), /retired/);
  await outbox.expire(prepared.outboxId, 356n); outbox = await openOutbox(backend, pair.host, keyring(), random);
  assert.equal(await outbox.gc(656n, 2), 2); pair.host.close(); pair.provider.close();
});

test("linked successor expiry atomically unlinks before restart", async () => {
  const pair = await transports(); const backend = new StrictMemoryBackend(); const random = nonceSource(); let outbox = await openOutbox(backend, pair.host, keyring(), random);
  const entry = await preparedEntry(pair.host); const prepared = await outbox.prepare({ entry }); const token = exactResumeToken(pair.host, entry, 2);
  const installed = await outbox.installSuccessor(prepared.outboxId, progress(entry[6], 1), token, 2); await outbox.confirmAck(prepared.outboxId, installed.responseHash);
  const successor = await outbox.prepareSuccessor(prepared.outboxId, { entry: await successorEntry(entry, token, 2) }); await outbox.expire(successor.outboxId, 356n);
  outbox = await openOutbox(backend, pair.host, keyring(), random); assert.throws(() => outbox.retry(successor.outboxId, 0n), /retired/);
  await outbox.expire(prepared.outboxId, 356n); outbox = await openOutbox(backend, pair.host, keyring(), random); assert.equal(backend.records.size, 2);
  pair.host.close(); pair.provider.close();
});

test("normal and successor prepares cannot race into the same live operation generation", async () => {
  const pair = await transports(); const backend = new StrictMemoryBackend(); const random = nonceSource(); let outbox = await openOutbox(backend, pair.host, keyring(), random);
  const entry = await preparedEntry(pair.host); const prepared = await outbox.prepare({ entry }); const token = exactResumeToken(pair.host, entry, 2);
  const installed = await outbox.installSuccessor(prepared.outboxId, progress(entry[6], 1), token, 2); await outbox.confirmAck(prepared.outboxId, installed.responseHash);
  const successor = await successorEntry(entry, token, 2); const normal = { ...await preparedEntry(pair.host, 1, { outbox: 0x68, request: 0x58, operation: 0x44 }), 8: 1 } as HostOutboxEntryV1;
  const raced = await Promise.allSettled([outbox.prepareSuccessor(prepared.outboxId, { entry: successor }), outbox.prepare({ entry: normal })]);
  assert.equal(raced.filter(({ status }) => status === "fulfilled").length, 1); assert.equal(raced.filter(({ status }) => status === "rejected").length, 1);
  outbox = await openOutbox(backend, pair.host, keyring(), random); assert.ok(outbox); pair.host.close(); pair.provider.close();
});

test("cancelled, expired, skipped-cursor, and wrong-generation resume authorities fail closed", async () => {
  const pair = await transports(); const make = async (outboxByte: number, requestByte: number, operationByte: number) => {
    const backend = new StrictMemoryBackend(); const outbox = await openOutbox(backend, pair.host); const entry = await preparedEntry(pair.host, 1, { outbox: outboxByte, request: requestByte, operation: operationByte });
    return { outbox, entry, prepared: await outbox.prepare({ entry }) };
  };
  {
    const { outbox, entry, prepared } = await make(0x61, 0x51, 0x41);
    await assert.rejects(outbox.installSuccessor(prepared.outboxId, progress(entry[6], 1), exactResumeToken(pair.host, entry, 2, true), 2), (error) => error instanceof BrowserOutboxError && error.code === "HOST_OUTBOX_BINDING_INVALID");
  }
  {
    const { entry } = await make(0x62, 0x52, 0x42); const token = decodeHostV2("ResumeTokenV1", exactResumeToken(pair.host, entry, 2)).value as any; token[12] = 229;
    assert.throws(() => encodeHostV2("ResumeTokenV1", token), /resume-validity-at-most-128/);
  }
  {
    const { outbox, entry, prepared } = await make(0x63, 0x53, 0x43);
    await assert.rejects(outbox.installSuccessor(prepared.outboxId, progress(entry[6], 1), exactResumeToken(pair.host, entry, 3), 3), (error) => error instanceof BrowserOutboxError && error.code === "HOST_OUTBOX_BINDING_INVALID");
  }
  {
    const { outbox, entry, prepared } = await make(0x64, 0x54, 0x44); const token = decodeHostV2("ResumeTokenV1", exactResumeToken(pair.host, entry, 2)).value as any; token[10] = 2;
    await assert.rejects(outbox.installSuccessor(prepared.outboxId, progress(entry[6], 1), encodeHostV2("ResumeTokenV1", token), 2), (error) => error instanceof BrowserOutboxError && error.code === "HOST_OUTBOX_BINDING_INVALID");
  }
  pair.host.close(); pair.provider.close();
});

test("authenticated MessagePort negotiation owns the remote offer and pending operations fail closed", async () => {
  const connected = await transports();
  assert.deepEqual(connected.host.negotiation.features, HOST_V2_FEATURE_IDS);
  const receiveAbort = new AbortController(); const pendingAbort = connected.host.receive("EventV2", { signal: receiveAbort.signal, timeoutMs: 1_000 }); receiveAbort.abort();
  await assert.rejects(pendingAbort, (error) => error instanceof BrowserHostV2TransportError && error.code === "BROWSER_OPERATION_ABORTED");
  await assert.rejects(connected.host.receive("EventV2", { timeoutMs: 5 }), (error) => error instanceof BrowserHostV2TransportError && error.code === "BROWSER_OPERATION_TIMEOUT");
  const pendingClose = connected.host.receive("EventV2", { timeoutMs: 1_000 }); connected.provider.close();
  await assert.rejects(pendingClose, (error) => error instanceof BrowserHostV2TransportError && error.code === "BROWSER_TRANSPORT_CLOSED");
  connected.host.close();

  const single = new MessageChannel(); const hostKey = await peer("host-timeout"); const remote = await peer("provider-timeout");
  await assert.rejects(BrowserHostV2Transport.connect(single.port1, hostKey.peer, remote.peer, () => {}, offer(), negotiationSigner(hostKey), { timeoutMs: 5 }), (error) => error instanceof BrowserHostV2TransportError && error.code === "BROWSER_OPERATION_TIMEOUT"); single.port2.close();

  const hostile = new MessageChannel(); const victimKey = await peer("host-victim"); const claimed = await peer("provider-claimed");
  const victim = BrowserHostV2Transport.connect(hostile.port1, victimKey.peer, claimed.peer, () => {}, offer(), negotiationSigner(victimKey), { timeoutMs: 1_000 });
  await new Promise<void>((resolve) => { hostile.port2.onmessage = () => resolve(); hostile.port2.start(); });
  const remoteOffer = offer(); hostile.port2.postMessage({ version: 2, channel: claimed.peer.channel, source: claimed.peer.source, target: victimKey.peer.source, kind: "offer", providerId: claimed.peer.providerId, endpointHash: claimed.peer.endpointHash, acknowledgementPublicKey: claimed.peer.acknowledgementPublicKey, offer: { protocol: remoteOffer.protocol, major: remoteOffer.major, minors: remoteOffer.minors, genesis: remoteOffer.genesis, finalizedSpecVersion: remoteOffer.finalizedSpecVersion, finalizedTransactionVersion: remoteOffer.finalizedTransactionVersion, registrySha256: remoteOffer.registrySha256, features: remoteOffer.features }, signature: new Uint8Array(64) });
  await assert.rejects(victim, (error) => error instanceof BrowserHostV2TransportError && error.code === "BROWSER_PEER_REJECTED"); hostile.port2.close();
});

test("reordered signed acknowledgement retains its tuple and duplicate or mismatched acknowledgement fails closed", async () => {
  const connectReordered = async (mode: "valid" | "duplicate" | "mismatch"): Promise<BrowserHostV2Transport> => {
    const channel = new MessageChannel(); const host = await peer(`reordered-host-${mode}`); const provider = await peer(`reordered-provider-${mode}`);
    const connected = BrowserHostV2Transport.connect(channel.port1, host.peer, provider.peer, () => {}, offer(), negotiationSigner(host), { timeoutMs: 1_000 });
    await new Promise<void>((resolve) => { channel.port2.addEventListener("message", () => resolve(), { once: true }); channel.port2.start(); });
    const ack = await signedEnvelope(provider, host.peer, "negotiated", mode === "mismatch" ? new Uint8Array(32).fill(0x99) : undefined);
    channel.port2.postMessage(ack);
    if (mode === "duplicate") channel.port2.postMessage(ack);
    channel.port2.postMessage(await signedEnvelope(provider, host.peer, "offer"));
    try { return await connected; } finally { channel.port2.close(); }
  };
  const valid = await connectReordered("valid"); assert.ok(equal(valid.binding.negotiatedTuple, await tupleDigest(offer()))); valid.close();
  await assert.rejects(connectReordered("duplicate"), (error) => error instanceof BrowserHostV2TransportError && error.code === "BROWSER_MESSAGE_INVALID");
  await assert.rejects(connectReordered("mismatch"), (error) => error instanceof BrowserHostV2TransportError && error.code === "BROWSER_PEER_REJECTED");
});

test("transport enforces cap, credits, abort and remote-close for pending send", async () => {
  const connected = await transports(); const canonical = encodeHostV2("RequestId", new Uint8Array(16));
  await assert.rejects(connected.host.send("Bytes4MiB", new Uint8Array(4_194_305)), (error) => error instanceof BrowserHostV2TransportError && error.code === "BROWSER_MESSAGE_TOO_LARGE");
  const sends = Array.from({ length: BROWSER_HOST_V2_WINDOW }, () => connected.host.send("RequestId", canonical, { timeoutMs: 1_000 }));
  await Promise.all(sends); assert.deepEqual(await connected.provider.receive("RequestId"), canonical);
  const abort = new AbortController(); const receive = connected.host.receive("EventV2", { signal: abort.signal }); abort.abort(); await assert.rejects(receive, /aborted/);
  const pending = connected.host.receive("EventV2", { timeoutMs: 1_000 }); connected.provider.close(); await assert.rejects(pending, /closed/); connected.host.close();

  const abortPair = await transports(); const sendAbort = new AbortController();
  const pendingCredit = abortPair.host.send("RequestId", canonical, { signal: sendAbort.signal, timeoutMs: 1_000 }); sendAbort.abort();
  await assert.rejects(pendingCredit, (error) => error instanceof BrowserHostV2TransportError && error.code === "BROWSER_OPERATION_ABORTED");
  abortPair.provider.close();

  const closePair = await transports(); const pendingCloseCredit = closePair.host.send("RequestId", canonical, { timeoutMs: 1_000 }); closePair.provider.close();
  await assert.rejects(pendingCloseCredit, (error) => error instanceof BrowserHostV2TransportError && error.code === "BROWSER_TRANSPORT_CLOSED"); closePair.host.close();
});

test("real XChaCha20-Poly1305 matches the frozen Rust envelope and rotates keys", async () => {
  const cryptoVector = JSON.parse(readFileSync(resolve(root, "docs/specs/host-outbox-v1.vectors.json"), "utf8")).base_vector.crypto;
  const fixtureContext: BrowserHostOutboxContextV1 = { profileId: new Uint8Array(32).fill(0x11), registryHash: new Uint8Array(32).fill(0x11), genesisHash: new Uint8Array(32).fill(0x22), negotiatedTuple: new Uint8Array(32).fill(0x33), providerId: new Uint8Array(32).fill(0x11), providerEndpointHash: new Uint8Array(32).fill(0x22) };
  const cipher = new BrowserXChaCha20Poly1305(fixtureContext, new BrowserHostOutboxKeyRingV1(1, new Map([[1, bytes(cryptoVector.key_hex)]])), globalThis.crypto, () => bytes(cryptoVector.nonce_hex));
  const sealed = await cipher.seal(new Uint8Array(16).fill(0x44), bytes(cryptoVector.plaintext_cbor_hex));
  assert.equal(Buffer.from(sealed.ciphertext).toString("hex"), cryptoVector.envelope_hex);
  assert.equal(Buffer.from(await cipher.open(new Uint8Array(16).fill(0x44), 1, sealed.ciphertext)).toString("hex"), cryptoVector.plaintext_cbor_hex);
  const corrupt = sealed.ciphertext.slice(); corrupt[corrupt.length - 1] ^= 1; await assert.rejects(cipher.open(new Uint8Array(16).fill(0x44), 1, corrupt), /authentication/);

  const connected = await transports(); const backend = new StrictMemoryBackend(); const random = nonceSource(); let outbox = await openOutbox(backend, connected.host, keyring(), random); const entry = await preparedEntry(connected.host);
  const prepared = await outbox.prepare({ entry }); assert.equal([...backend.records.values()][0]!.keyVersion, 1);
  outbox = await openOutbox(backend, connected.host, keyring(2), random); await outbox.markSent(prepared.outboxId); assert.equal([...backend.records.values()][0]!.keyVersion, 2);
  await openOutbox(backend, connected.host, keyring(2, false), random); connected.host.close(); connected.provider.close();
});

test("outbox IDs are independent while concurrent provider reads do not collide on absent operation IDs", async () => {
  const connected = await transports(); const backend = new StrictMemoryBackend(); const outbox = await openOutbox(backend, connected.host);
  const first = await preparedEntry(connected.host, 1, { vector: "1011-positive", outbox: 0x61, request: 0x51, expected: 2 });
  const second = await preparedEntry(connected.host, 1, { vector: "1011-positive", outbox: 0x62, request: 0x52, expected: 2 });
  const [left, right] = await Promise.all([outbox.prepare({ entry: first }), outbox.prepare({ entry: second })]);
  assert.ok(equal(left.operationId, new Uint8Array(16))); assert.ok(equal(right.operationId, new Uint8Array(16))); assert.equal(backend.records.size, 2);

  const duplicateOutbox = await preparedEntry(connected.host, 1, { vector: "1011-positive", outbox: 0x61, request: 0x53, expected: 2 });
  await assert.rejects(outbox.prepare({ entry: duplicateOutbox }), (error) => error instanceof BrowserOutboxError && error.code === "HOST_OUTBOX_STATE_INVALID");
  const writeA = await preparedEntry(connected.host, 1, { outbox: 0x71, request: 0x71, operation: 0x41 });
  const writeB = await preparedEntry(connected.host, 1, { outbox: 0x72, request: 0x72, operation: 0x41 });
  const results = await Promise.allSettled([outbox.prepare({ entry: writeA }), outbox.prepare({ entry: writeB })]);
  assert.equal(results.filter(({ status }) => status === "fulfilled").length, 1); assert.equal(results.filter(({ status }) => status === "rejected").length, 1);
  connected.host.close(); connected.provider.close();
});

test("concurrent prepares reserve record and encrypted-byte capacity and release it after failure", async () => {
  const connected = await transports();
  const entries = await Promise.all([
    preparedEntry(connected.host, 1, { vector: "1011-positive", outbox: 0x31, request: 0x31, expected: 2 }),
    preparedEntry(connected.host, 1, { vector: "1011-positive", outbox: 0x32, request: 0x32, expected: 2 }),
  ]);

  const recordBackend = new StrictMemoryBackend(); recordBackend.putDelayMs = 20;
  const recordBound = await BrowserHostOutboxV1.open(recordBackend, context(connected.host), keyring(), { records: 1, random: nonceSource() });
  const recordResults = await Promise.allSettled(entries.map((entry) => recordBound.prepare({ entry })));
  assert.equal(recordResults.filter(({ status }) => status === "fulfilled").length, 1);
  assert.equal(recordResults.filter(({ status }) => status === "rejected").length, 1);
  assert.equal(recordBackend.putStarts, 1, "a record-limit loser reached durable commit");
  assert.equal(recordBackend.records.size, 1);

  const probeBackend = new StrictMemoryBackend();
  const probe = await BrowserHostOutboxV1.open(probeBackend, context(connected.host), keyring(), { random: nonceSource() });
  await probe.prepare({ entry: entries[0]! });
  const oneRecordBytes = [...probeBackend.records.values()][0]!.ciphertext.length;
  const byteBackend = new StrictMemoryBackend(); byteBackend.putDelayMs = 20;
  const byteBound = await BrowserHostOutboxV1.open(byteBackend, context(connected.host), keyring(), { records: 2, bytes: oneRecordBytes, random: nonceSource() });
  const byteResults = await Promise.allSettled(entries.map((entry) => byteBound.prepare({ entry })));
  assert.equal(byteResults.filter(({ status }) => status === "fulfilled").length, 1);
  assert.equal(byteResults.filter(({ status }) => status === "rejected").length, 1);
  assert.equal(byteBackend.putStarts, 1, "an encrypted-byte-limit loser reached durable commit");
  assert.equal(byteBackend.records.size, 1);

  const failureBackend = new StrictMemoryBackend(); failureBackend.failBeforeCommit = true;
  const afterFailure = await BrowserHostOutboxV1.open(failureBackend, context(connected.host), keyring(), { records: 1, random: nonceSource() });
  await assert.rejects(afterFailure.prepare({ entry: entries[0]! }), (error) => error instanceof BrowserOutboxError && error.code === "HOST_OUTBOX_UNAVAILABLE");
  await afterFailure.prepare({ entry: entries[1]! });
  assert.equal(failureBackend.putStarts, 2); assert.equal(failureBackend.records.size, 1);
  connected.host.close(); connected.provider.close();
});

test("forged durable entry fields fail before any provider-visible send", async () => {
  const connected = await transports(); const backend = new StrictMemoryBackend(); const outbox = await openOutbox(backend, connected.host); const valid = await preparedEntry(connected.host);
  const forgeries: HostOutboxEntryV1[] = [
    { ...valid, 5: new Uint8Array(32) }, { ...valid, 10: new Uint8Array(32) }, { ...valid, 11: new Uint8Array(32) },
    { ...valid, 12: new Uint8Array(32) }, { ...valid, 13: new Uint8Array(32) }, { ...valid, 14: new Uint8Array(32) },
    { ...valid, 15: 0 }, { ...valid, 19: 200 }, { ...valid, 20: 2 },
  ];
  for (const entry of forgeries) await assert.rejects(outbox.prepare({ entry }), (error) => error instanceof BrowserOutboxError && ["HOST_OUTBOX_BINDING_INVALID", "HOST_OUTBOX_STATE_INVALID"].includes(error.code));
  assert.equal(backend.records.size, 0); await assert.rejects(connected.provider.receive("RequestV2", { timeoutMs: 5 }), /timed out/); connected.host.close(); connected.provider.close();
});

test("terminal payload must match the exact requested operation result", async () => {
  const pair = await transports(); const backend = new StrictMemoryBackend(); const outbox = await openOutbox(backend, pair.host); const entry = { ...await preparedEntry(pair.host), 15: 2 } as HostOutboxEntryV1;
  const durable = new DurableBrowserHostV2(pair.host, outbox); const sent = await durable.prepareAndSend({ entry }); await pair.provider.receive("RequestV2"); await pair.provider.receive("ProviderCapabilityV1");
  await pair.provider.send("EventV2", accepted(sent.requestId)); await durable.receiveEvent(200n);
  const hash = new Uint8Array(32).fill(0x61); const wrongIdentityResult = encodeHostV2("ResultEventV2", { 0: 2, 1: sent.requestId, 2: 1, 3: 2, 4: { 0: hash, 1: 0, 2: { 0: 0, 1: hash } } } as any);
  await pair.provider.send("EventV2", wrongIdentityResult); await assert.rejects(durable.receiveEvent(200n), /payload mismatches/); pair.host.close(); pair.provider.close();
});

test("generated errors and sequential provider-byte durable requests retain exact bindings", async () => {
  const pair = await transports(); const backend = new StrictMemoryBackend(); const outbox = await openOutbox(backend, pair.host);
  const providerRead = await preparedEntry(pair.host, 1, { vector: "1011-positive", outbox: 0x61, request: 0x51, expected: 2 });
  await outbox.prepare({ entry: providerRead });
  const durable = new DurableBrowserHostV2(pair.host, outbox);
  const first = await durable.resumeAndSend(providerRead[1], 100n); await pair.provider.receive("RequestV2"); await pair.provider.receive("ProviderCapabilityV1");
  await pair.provider.send("EventV2", accepted(first.requestId)); await durable.receiveEvent(200n);
  const exactError = encodeHostV2Value({ 0: 2, 1: first.requestId, 2: 1, 3: 3, 4: { 0: 108, 1: "REQUEST_NOT_FOUND", 2: false, 3: {} } });
  await pair.provider.send("EventV2", exactError); assert.equal((await durable.receiveEvent(200n)).terminal, true); await pair.provider.receive("ResponseAckV1");

  const secondEntry = await preparedEntry(pair.host, 1, { outbox: 0x62, request: 0x52, operation: 0x42, expected: 4 });
  const second = await durable.prepareAndSend({ entry: secondEntry }); await pair.provider.receive("RequestV2"); await pair.provider.receive("ProviderCapabilityV1");
  await pair.provider.send("EventV2", accepted(second.requestId)); await durable.receiveEvent(201n);
  await pair.provider.send("EventV2", cancelled(second.requestId, 1)); assert.equal((await durable.receiveEvent(201n)).terminal, true); await pair.provider.receive("ResponseAckV1");
  pair.host.close(); pair.provider.close();
});

test("private browser adapter streams the exact object.put vector through MessagePort and the Rust byte bridge", async () => {
  const pair = await transports(); const outbox = await openOutbox(new StrictMemoryBackend(), pair.host);
  const vector = frozen.vectors.find((candidate: any) => candidate.id === "1010-positive");
  const request = bytes(vector.wire_hex); const frame = decodeHostV2("RequestV2", request).value as any;
  const authorityVector = protocol.vectors.find((candidate: any) => candidate.id === "provider-capability-v1");
  const capability = decodeHostV2("ProviderCapabilityV1", bytes(authorityVector.canonical_cbor_hex)).value as any;
  capability[1] = pair.host.binding.registryHash; capability[2] = pair.host.binding.genesisHash;
  capability[3] = frame[4]; capability[5] = frame[2]; capability[6] = frame[8][0];
  capability[8] = pair.host.binding.providerId; capability[9] = [frame[3]]; capability[12] = 100; capability[13] = 228;
  const authority = encodeHostV2("ProviderCapabilityV1", capability);
  let acknowledged = false; let bridgedRequest: Uint8Array | undefined; let bridgedAuthority: Uint8Array | undefined;
  const bridge: PrivateBrowserRustProviderBridgeV2 = {
    async *dispatch(input) {
      bridgedRequest = input.request.slice(); bridgedAuthority = input.authority.slice();
      yield { event: accepted(frame[1]), terminalBlock: 200n };
      const chunks: Uint8Array[] = []; for await (const exact of input.upload ?? []) {
        const chunk = decodeHostV2("ProviderTransferChunkV1", exact).value; chunks.push(chunk[3]);
        assert.equal(Buffer.from(chunk[4]).toString("hex"), "9ac3628f6c9087cc04c77a07a06dc41aa7aa8ff8439b43354754cb41d2803436");
      }
      assert.deepEqual(chunks, [Uint8Array.of(0xab)]);
      yield { event: encodeHostV2Value({ 0: 2, 1: frame[1], 2: 1, 3: 1, 4: { 0: 1, 2: 1 } }), terminalBlock: 200n };
      yield { event: encodeHostV2Value({ 0: 2, 1: frame[1], 2: 2, 3: 2, 4: { 0: { 0: pair.host.binding.providerId, 1: frame[8][1], 2: 1, 3: new Uint8Array(64) }, 1: true, 2: { 0: 200, 1: new Uint8Array(32).fill(0x91) } } }), terminalBlock: 200n };
    },
    async acknowledge(exact) { decodeHostV2("ResponseAckV1", exact); acknowledged = true; },
  };
  const abort = new AbortController(); const pump = runPrivateBrowserRustProviderV2(pair.provider, bridge, { signal: abort.signal }).catch(() => undefined);
  const host = new PrivateDurableBrowserHostV2({
    durable: new DurableBrowserHostV2(pair.host, outbox), outbox,
    finality: { async finalized() { return { number: 100n, hash: new Uint8Array(32).fill(0x42) }; } },
    authority: { async resolve() { return authority.slice(); } },
    outboxIds: { next() { return new Uint8Array(16).fill(0x71); } },
  });
  const result = await host.invoke("storage.object.put", request, { cid: frame[8][1], length: 1n, bytes: (async function* () { yield Uint8Array.of(0xab); })() });
  assert.equal((result.value as any).publishable, true); assert.deepEqual(bridgedRequest, request); assert.deepEqual(bridgedAuthority, authority); assert.equal(acknowledged, true);
  abort.abort(); await pump; pair.host.close(); pair.provider.close();
});

test("provider upload emits a fifth chunk only after chunks_acked advances", async () => {
  const pair = await transports(); const outbox = await openOutbox(new StrictMemoryBackend(), pair.host);
  const vector = frozen.vectors.find((candidate: any) => candidate.id === "1010-positive");
  const frame = decodeHostV2("RequestV2", bytes(vector.wire_hex)).value as any; const length = 1_048_577;
  frame[8][2] = length; const request = encodeHostV2("RequestV2", frame);
  const authorityVector = protocol.vectors.find((candidate: any) => candidate.id === "provider-capability-v1");
  const capability = decodeHostV2("ProviderCapabilityV1", bytes(authorityVector.canonical_cbor_hex)).value as any;
  capability[1] = pair.host.binding.registryHash; capability[2] = pair.host.binding.genesisHash; capability[3] = frame[4];
  capability[5] = frame[2]; capability[6] = frame[8][0]; capability[8] = pair.host.binding.providerId; capability[9] = [1010];
  capability[11] = length; capability[12] = 100; capability[13] = 228; const authority = encodeHostV2("ProviderCapabilityV1", capability);
  let fifthArrivedBeforeAck = true;
  const bridge: PrivateBrowserRustProviderBridgeV2 = {
    async *dispatch(input) {
      yield { event: accepted(frame[1]), terminalBlock: 100n };
      const iterator = input.upload![Symbol.asyncIterator]();
      for (let index = 0; index < 4; index += 1) assert.equal((await iterator.next()).done, false);
      const fifth = iterator.next();
      fifthArrivedBeforeAck = await Promise.race([fifth.then(() => true), new Promise<false>((resolve) => setTimeout(() => resolve(false), 10))]);
      yield { event: encodeHostV2Value({ 0: 2, 1: frame[1], 2: 1, 3: 1, 4: { 0: 262_144, 2: 1 } }), terminalBlock: 100n };
      assert.equal((await fifth).done, false);
      yield { event: encodeHostV2Value({ 0: 2, 1: frame[1], 2: 2, 3: 1, 4: { 0: length, 2: 5 } }), terminalBlock: 100n };
      yield { event: encodeHostV2Value({ 0: 2, 1: frame[1], 2: 3, 3: 3, 4: { 0: 108, 1: "REQUEST_NOT_FOUND", 2: false, 3: {} } }), terminalBlock: 100n };
    }, async acknowledge(exact) { decodeHostV2("ResponseAckV1", exact); },
  };
  const abort = new AbortController(); const pump = runPrivateBrowserRustProviderV2(pair.provider, bridge, { signal: abort.signal }).catch(() => undefined);
  const host = new PrivateDurableBrowserHostV2({
    durable: new DurableBrowserHostV2(pair.host, outbox), outbox,
    finality: { async finalized() { return { number: 100n, hash: new Uint8Array(32).fill(0x42) }; } },
    authority: { async resolve() { return authority; } }, outboxIds: { next() { return new Uint8Array(16).fill(0x72); } },
  });
  const payload = new Uint8Array(length).fill(0x5a);
  assert.equal((await host.invoke("storage.object.put", request, { cid: frame[8][1], length: BigInt(length), bytes: (async function* () { yield payload; })() })).error?.code, 108);
  assert.equal(fifthArrivedBeforeAck, false);
  abort.abort(); await pump; pair.host.close(); pair.provider.close();
});

test("terminal error seals a delayed upload source before it can emit a post-terminal chunk", async () => {
  const pair = await transports(); const outbox = await openOutbox(new StrictMemoryBackend(), pair.host);
  const vector = frozen.vectors.find((candidate: any) => candidate.id === "1010-positive");
  const frame = decodeHostV2("RequestV2", bytes(vector.wire_hex)).value as any; frame[8][2] = 1;
  const request = encodeHostV2("RequestV2", frame); const authority = exactCapability(pair.host, frame, 1);
  let release!: () => void; const delayed = new Promise<void>((resolve) => { release = resolve; }); let sourceClosed = false;
  const bridge: PrivateBrowserRustProviderBridgeV2 = {
    async *dispatch() {
      yield { event: accepted(frame[1]), terminalBlock: 100n };
      yield { event: encodeHostV2Value({ 0: 2, 1: frame[1], 2: 1, 3: 3, 4: { 0: 108, 1: "REQUEST_NOT_FOUND", 2: false, 3: {} } }), terminalBlock: 100n };
    }, async acknowledge(exact) { decodeHostV2("ResponseAckV1", exact); },
  };
  const abort = new AbortController(); const pump = runPrivateBrowserRustProviderV2(pair.provider, bridge, { signal: abort.signal }).catch(() => undefined);
  const host = new PrivateDurableBrowserHostV2({
    durable: new DurableBrowserHostV2(pair.host, outbox), outbox,
    finality: { async finalized() { return { number: 100n, hash: new Uint8Array(32).fill(0x42) }; } },
    authority: { async resolve() { return authority; } }, outboxIds: { next() { return new Uint8Array(16).fill(0x73); } },
  });
  const invocation = host.invoke("storage.object.put", request, { cid: frame[8][1], length: 1n, bytes: (async function* () { try { await delayed; yield Uint8Array.of(0xab); } finally { sourceClosed = true; } })() });
  assert.equal((await invocation).error?.code, 108); release();
  await new Promise((resolve) => setTimeout(resolve, 10)); assert.equal(sourceClosed, true);
  await assert.rejects(pair.provider.receive("ProviderTransferChunkV1", { timeoutMs: 10 }), /timed out/, "post-terminal upload chunk escaped sealing");
  abort.abort(); await pump; pair.host.close(); pair.provider.close();
});

test("AbortSignal wakes a provider upload blocked on the four-chunk acknowledgement window", async () => {
  const pair = await transports(); const outbox = await openOutbox(new StrictMemoryBackend(), pair.host); const length = 1_310_720;
  const vector = frozen.vectors.find((candidate: any) => candidate.id === "1010-positive");
  const frame = decodeHostV2("RequestV2", bytes(vector.wire_hex)).value as any; frame[8][2] = length;
  const request = encodeHostV2("RequestV2", frame); const authority = exactCapability(pair.host, frame, length);
  let fourReceived!: () => void; const firstFour = new Promise<void>((resolve) => { fourReceived = resolve; }); let sourceClosed = false;
  const bridge: PrivateBrowserRustProviderBridgeV2 = {
    async *dispatch(input) {
      yield { event: accepted(frame[1]), terminalBlock: 100n }; const iterator = input.upload![Symbol.asyncIterator]();
      for (let index = 0; index < 4; index += 1) assert.equal((await iterator.next()).done, false);
      fourReceived(); await new Promise<void>((_resolve, reject) => input.signal!.addEventListener("abort", () => reject(input.signal!.reason), { once: true }));
    }, async acknowledge() {},
  };
  const abort = new AbortController(); const pump = runPrivateBrowserRustProviderV2(pair.provider, bridge, { signal: abort.signal }).catch(() => undefined);
  const host = new PrivateDurableBrowserHostV2({
    durable: new DurableBrowserHostV2(pair.host, outbox), outbox,
    finality: { async finalized() { return { number: 100n, hash: new Uint8Array(32).fill(0x42) }; } },
    authority: { async resolve() { return authority; } }, outboxIds: { next() { return new Uint8Array(16).fill(0x74); } },
  });
  const invocation = host.invoke("storage.object.put", request, { cid: frame[8][1], length: BigInt(length), bytes: (async function* () { try { yield new Uint8Array(length).fill(0x5a); } finally { sourceClosed = true; } })() }, abort.signal);
  await firstFour; abort.abort(new Error("host cancelled")); await assert.rejects(invocation, /host cancelled|aborted/);
  await new Promise((resolve) => setTimeout(resolve, 10)); assert.equal(sourceClosed, true, "blocked upload iterator was not closed after abort");
  await pump; pair.host.close(); pair.provider.close();
});

test("terminal install followed by ACK-send failure still seals a blocked upload", async () => {
  const pair = await transports(); const backend = new StrictMemoryBackend(); const outbox = await openOutbox(backend, pair.host); const length = 1_310_720;
  const vector = frozen.vectors.find((candidate: any) => candidate.id === "1010-positive"); const frame = decodeHostV2("RequestV2", bytes(vector.wire_hex)).value as any;
  frame[8][2] = length; const request = encodeHostV2("RequestV2", frame); const authority = exactCapability(pair.host, frame, length); let sourceClosed = false;
  const host = new PrivateDurableBrowserHostV2({
    durable: new DurableBrowserHostV2(pair.host, outbox), outbox,
    finality: { async finalized() { return { number: 100n, hash: new Uint8Array(32).fill(0x42) }; } },
    authority: { async resolve() { return authority; } }, outboxIds: { next() { return new Uint8Array(16).fill(0x79); } },
  });
  const invocation = host.invoke("storage.object.put", request, { cid: frame[8][1], length: BigInt(length), bytes: (async function* () { try { yield new Uint8Array(length).fill(0x5a); } finally { sourceClosed = true; } })() });
  await pair.provider.receive("RequestV2"); await pair.provider.receive("ProviderCapabilityV1"); await pair.provider.send("EventV2", accepted(frame[1]));
  for (let index = 0; index < 4; index += 1) await pair.provider.receive("ProviderTransferChunkV1");
  await pair.provider.send("EventV2", encodeHostV2Value({ 0: 2, 1: frame[1], 2: 1, 3: 3, 4: { 0: 108, 1: "REQUEST_NOT_FOUND", 2: false, 3: {} } })); pair.provider.close();
  await assert.rejects(invocation, /closed|ACK|transport/i); await new Promise((resolve) => setTimeout(resolve, 10));
  assert.equal(sourceClosed, true); assert.deepEqual(outbox.installedAck(new Uint8Array(16).fill(0x79)).responseHash.length, 32); pair.host.close();
});

test("private browser adapter registry covers all 26 storage and eight identity/signing frozen vectors", () => {
  const positives = frozen.vectors.filter((vector: any) => vector.kind === "operation-schema-positive");
  assert.equal(positives.length, 34);
  assert.equal(positives.filter((vector: any) => vector.operation.startsWith("storage.")).length, 26);
  assert.equal(positives.filter((vector: any) => vector.operation.startsWith("identity.") || vector.operation === "transaction.sign").length, 8);
  for (const vector of positives) {
    const binding = HOST_V2_OPERATION_BINDINGS[vector.operation as keyof typeof HOST_V2_OPERATION_BINDINGS];
    assert.ok(binding, `${vector.operation} is absent from the private browser binding`);
    const exact = bytes(vector.wire_hex); const decoded = decodeHostV2(binding.frame, exact);
    assert.deepEqual(encodeHostV2(binding.frame, decoded.value), exact, `${vector.operation} changed its cross-language bytes`);
  }
});

test("six-authority router keeps all 30 non-provider operations and grants off the provider bridge", async () => {
  const routed = { provider: [] as string[], commons: [] as string[], keys: [] as string[], runtimeIdentity: [] as string[], hostIdentity: [] as string[], signing: [] as string[] };
  const authority: PrivateFinalizedHostAuthorityV2 = { number: 100n, hash: new Uint8Array(32).fill(0x42), proof: Uint8Array.of(0xa1) };
  const bridge = (trace: string[]) => ({
    async finalizedAuthority() { return authority; },
    async *dispatch(input: any) {
      trace.push(input.operation); const request = decodeHostV2("RequestV2", input.request).value as any;
      assert.equal(input.authority, authority, "request and finalized authority were joined or regenerated");
      yield { event: accepted(request[1]), terminalBlock: 100n };
      yield { event: encodeHostV2Value({ 0: 2, 1: request[1], 2: 1, 3: 3, 4: { 0: 108, 1: "REQUEST_NOT_FOUND", 2: false, 3: {} } }), terminalBlock: 100n };
    },
  });
  const router = new PrivateOriginBrowserRouterV2({
    provider: { async invoke(operation) { routed.provider.push(operation); throw new Error("provider bridge must not receive non-provider traffic"); } },
    commons: bridge(routed.commons), keystore: bridge(routed.keys), identityRuntime: bridge(routed.runtimeIdentity),
    identityHost: bridge(routed.hostIdentity), signing: bridge(routed.signing),
  });
  const positives = frozen.vectors.filter((vector: any) => vector.kind === "operation-schema-positive" && ![1010, 1011, 1012, 1014].includes(Number(vector.id.slice(0, 4))));
  for (const vector of positives) {
    const result = await router.invoke(vector.operation, bytes(vector.wire_hex)); assert.equal(result.error?.code, 108);
  }
  assert.deepEqual(routed.provider, []);
  assert.deepEqual([routed.commons.length, routed.keys.length, routed.runtimeIdentity.length, routed.hostIdentity.length, routed.signing.length], [20, 2, 3, 4, 1]);
  assert.equal([...routed.runtimeIdentity, ...routed.hostIdentity, ...routed.signing].length, 8);
});

test("concrete Commons bridge publishes and resolves through exact native finality without provider traffic", async () => {
  const publishVector = frozen.vectors.find((candidate: any) => candidate.id === "1050-positive");
  const resolveVector = frozen.vectors.find((candidate: any) => candidate.id === "1051-positive");
  const publishFrame = decodeHostV2("RequestV2", bytes(publishVector.wire_hex)).value as any;
  const cid = publishFrame[8][1] as string;
  const digest = parseContentCid(cid).digest;
  const digestHex = `0x${Buffer.from(digest).toString("hex")}` as const;
  const nameHex = `0x${"22".repeat(32)}` as const;
  const bucketHex = `0x${"55".repeat(32)}` as const;
  const finalized99 = `0x${"42".repeat(32)}` as const;
  const finalized100 = `0x${"43".repeat(32)}` as const;
  const calls: { kind: string; at: string; target: string; payload: unknown }[] = [];
  const signer = { async accounts() { throw new Error("unused"); }, async sign() { throw new Error("unused"); } };
  const runtime = {
    async read(at: `0x${string}`, target: string, payload: Readonly<Record<string, unknown>>) {
      calls.push({ kind: "read", at, target, payload });
      if (target === "NamesApi.root_name_by_normalized_label") return { version: 1, value: nameHex };
      if (target === "NamesApi.resolve_content") return { version: 1, value: digestHex };
      if (target === "StorageProviderApi.canonical_manifest") return {
        version: 11, value: { manifest: digestHex, bucket_id: bucketHex, state: "Publishable", checkpoint: 77 },
      };
      if (target === "StorageProviderApi.checkpoint") return {
        version: 11, value: { bucket_id: bucketHex, commitment: { mmr_root: `0x${"66".repeat(32)}`, start_seq: 8, leaf_count: 3 },
          checkpoint_block: 77, primary_signers: 1, commitment_nonce: 70, replica_confirmations: [nameHex, bucketHex] },
      };
      throw new Error(`unexpected read ${target}`);
    },
    async prepare(at: `0x${string}`, target: string, payload: Readonly<Record<string, unknown>>) {
      calls.push({ kind: "prepare", at, target, payload });
      assert.equal(target, "Names.set_content");
      assert.deepEqual(payload, { name: nameHex, content: digestHex });
      return { async *signSubmitAndWatch(exactSigner: unknown) {
        assert.equal(exactSigner, signer); yield { type: "broadcast" as const };
        yield { type: "finalized" as const, blockHash: finalized100, transactionHash: `0x${"77".repeat(32)}` as const };
      } };
    },
  };
  const verified: string[] = [];
  const commons = new PrivateCordCommonsRuntimeBridgeV2({
    runtime, signer: signer as never,
    finality: {
      async finalized() { return { number: 99n, hash: finalized99, proof: Uint8Array.of(0xa1) }; },
      async verify(hash) { verified.push(hash); assert.equal(hash, finalized100); return { number: 100n, hash, proof: Uint8Array.of(0xa2) }; },
    },
    events: { async events(receipt) {
      assert.deepEqual(receipt, { blockHash: finalized100, transactionHash: `0x${"77".repeat(32)}` });
      return [{ pallet: "Names", event: "ContentSet", fields: { name: nameHex, present: true }, eventIndex: 9 }];
    } },
  });
  let providerTraffic = 0;
  const unreachable = { async finalizedAuthority() { throw new Error("unreachable authority"); }, async *dispatch() { throw new Error("unreachable dispatch"); } };
  const router = new PrivateOriginBrowserRouterV2({
    provider: { async invoke() { providerTraffic += 1; throw new Error("provider MessagePort received Commons traffic"); } },
    commons, keystore: unreachable, identityRuntime: unreachable, identityHost: unreachable, signing: unreachable,
  });
  const published = await router.invoke("storage.publish", bytes(publishVector.wire_hex));
  assert.equal(published.error, undefined); assert.equal((published.value as any).cid, cid);
  const resolved = await router.invoke("storage.resolve", bytes(resolveVector.wire_hex));
  assert.equal(resolved.error, undefined); assert.equal((resolved.value as any).cid, cid);
  assert.equal((resolved.value as any).checkpoint.from, 8n); assert.equal((resolved.value as any).checkpoint.to, 10n);
  assert.deepEqual(verified, [finalized100]); assert.equal(providerTraffic, 0);
  assert.deepEqual(calls.map(({ kind, at, target }) => `${kind}:${at}:${target}`), [
    `read:${finalized99}:StorageProviderApi.canonical_manifest`,
    `prepare:${finalized99}:Names.set_content`,
    `read:${finalized100}:NamesApi.resolve_content`,
    `read:${finalized99}:NamesApi.root_name_by_normalized_label`,
    `read:${finalized99}:NamesApi.resolve_content`,
    `read:${finalized99}:StorageProviderApi.canonical_manifest`,
    `read:${finalized99}:StorageProviderApi.checkpoint`,
  ]);
  const callCount = calls.length;
  for (const nonCanonicalCid of [
    `B${cid.slice(1).toUpperCase()}`,
    (() => {
      const alphabet = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
      const cidBytes = Uint8Array.from([1, 0x55, 0xa0, 0xe4, 0x02, 32, ...digest]);
      let value = 0n; for (const byte of cidBytes) value = value * 256n + BigInt(byte);
      let encoded = ""; while (value > 0n) { encoded = alphabet[Number(value % 58n)] + encoded; value /= 58n; }
      return `z${encoded}`;
    })(),
  ]) {
    const hostile = decodeHostV2("RequestV2", bytes(publishVector.wire_hex)).value as any;
    hostile[8][1] = nonCanonicalCid;
    assert.equal((await router.invoke("storage.publish", encodeHostV2("RequestV2", hostile))).error?.code, 204);
  }
  assert.equal(calls.length, callCount, "non-canonical CIDs reached the runtime transport");
});

test("real provider MessagePorts accept exactly the four provider-byte operations", async () => {
  const operations = ["1010-positive", "1011-positive", "1012-positive", "1014-positive"];
  const seen: number[] = [];
  for (const id of operations) {
    const pair = await transports(); const outbox = await openOutbox(new StrictMemoryBackend(), pair.host);
    const vector = frozen.vectors.find((candidate: any) => candidate.id === id); const request = bytes(vector.wire_hex);
    const frame = decodeHostV2("RequestV2", request).value as any;
    const authorityVector = protocol.vectors.find((candidate: any) => candidate.id === "provider-capability-v1");
    const capability = decodeHostV2("ProviderCapabilityV1", bytes(authorityVector.canonical_cbor_hex)).value as any;
    capability[1] = pair.host.binding.registryHash; capability[2] = pair.host.binding.genesisHash; capability[3] = frame[4];
    capability[5] = frame[2]; capability[6] = frame[8][0]; capability[8] = pair.host.binding.providerId; capability[9] = [frame[3]];
    capability[12] = 100; capability[13] = 228; const exactAuthority = encodeHostV2("ProviderCapabilityV1", capability);
    const bridge: PrivateBrowserRustProviderBridgeV2 = {
      async *dispatch(input) {
        seen.push(Number((decodeHostV2("RequestV2", input.request).value as any)[3]));
        yield { event: accepted(frame[1]), terminalBlock: 100n };
        yield { event: encodeHostV2Value({ 0: 2, 1: frame[1], 2: 1, 3: 3, 4: { 0: 108, 1: "REQUEST_NOT_FOUND", 2: false, 3: {} } }), terminalBlock: 100n };
      }, async acknowledge(exact) { decodeHostV2("ResponseAckV1", exact); },
    };
    const abort = new AbortController(); const pump = runPrivateBrowserRustProviderV2(pair.provider, bridge, { signal: abort.signal }).catch(() => undefined);
    const host = new PrivateDurableBrowserHostV2({
      durable: new DurableBrowserHostV2(pair.host, outbox), outbox,
      finality: { async finalized() { return { number: 100n, hash: new Uint8Array(32).fill(0x42) }; } },
      authority: { async resolve() { return exactAuthority; } }, outboxIds: { next() { return new Uint8Array(16).fill(Number(id.slice(2, 4))); } },
    });
    assert.equal((await host.invoke(vector.operation, request)).error?.code, 108);
    abort.abort(); await pump; pair.host.close(); pair.provider.close();
  }
  assert.deepEqual(seen, [1010, 1011, 1012, 1014]);
});

test("hostile provider spy rejects Identity before dispatch and observes no private payload", async () => {
  const pair = await transports(); const vector = frozen.vectors.find((candidate: any) => candidate.id === "1102-positive");
  const request = bytes(vector.wire_hex); const frame = decodeHostV2("RequestV2", request).value as any;
  const authorityVector = protocol.vectors.find((candidate: any) => candidate.id === "provider-capability-v1");
  const capability = decodeHostV2("ProviderCapabilityV1", bytes(authorityVector.canonical_cbor_hex)).value as any;
  capability[1] = pair.host.binding.registryHash; capability[2] = pair.host.binding.genesisHash; capability[3] = frame[4];
  capability[5] = frame[2]; capability[8] = pair.host.binding.providerId; capability[9] = [frame[3]];
  const authority = encodeHostV2("ProviderCapabilityV1", capability); let dispatches = 0;
  const spy: PrivateBrowserRustProviderBridgeV2 = {
    async *dispatch() { dispatches += 1; throw new Error("Identity reached provider dispatcher"); },
    async acknowledge() {},
  };
  const pump = assert.rejects(runPrivateBrowserRustProviderV2(pair.provider, spy), /non-provider Host-v2 request/);
  await Promise.all([pair.host.send("RequestV2", request), pair.host.send("ProviderCapabilityV1", authority)]);
  await pump; assert.equal(dispatches, 0);
  pair.host.close(); pair.provider.close();
});

test("storage resume validates exact ResumeToken and refuses unsafe intent replay", async () => {
  const unreachable = { async finalizedAuthority() { throw new Error("unreachable"); }, async *dispatch() { throw new Error("unreachable"); } };
  const router = new PrivateOriginBrowserRouterV2({
    provider: { async invoke() { throw new Error("unreachable"); } }, commons: unreachable, keystore: unreachable,
    identityRuntime: unreachable, identityHost: unreachable, signing: unreachable,
  });
  const storage = new PrivateDurableBrowserStorageV2(router); const execution = storage.start({} as any);
  const token = bytes(protocol.vectors.find((candidate: any) => candidate.id === "provider-resume-v1").canonical_cbor_hex);
  await assert.rejects(async () => { for await (const _event of execution.resume({ kind: "provider-token", token })) void _event; }, /cannot recover an exact durable continuation/);
});

test("StorageV2Execution.resume retains its exact continuation across failure after durable successor send", async () => {
  const frame = decodeHostV2("RequestV2", bytes(frozen.vectors.find((candidate: any) => candidate.id === "1010-positive").wire_hex)).value as any;
  const token = bytes(protocol.vectors.find((candidate: any) => candidate.id === "provider-resume-v1").canonical_cbor_hex);
  const continuation = { operation: "storage.object.put", request: encodeHostV2("RequestV2", frame), token, predecessorOutboxId: new Uint8Array(16).fill(1), cursor: 1, hostKeyId: new Uint8Array(32).fill(2) } as const;
  let resumed = 0; const provider = {
    async invoke(_operation: any, _request: any, _upload: any, _signal: any, onEvent: any) { onEvent?.(accepted(frame[1])); return { continuation }; },
    async resume(exact: any, _signal: any, onEvent: any) {
      assert.equal(exact, continuation); resumed += 1;
      if (resumed === 1) throw new Error("failure after durable successor send");
      onEvent?.(encodeHostV2Value({ 0: 2, 1: frame[1], 2: 1, 3: 3, 4: { 0: 108, 1: "REQUEST_NOT_FOUND", 2: false, 3: {} } }));
      return { error: { code: 108, name: "REQUEST_NOT_FOUND", retryable: false } };
    },
  };
  const unreachable = { async finalizedAuthority() { throw new Error("unreachable"); }, async *dispatch() { throw new Error("unreachable"); } };
  const storage = new PrivateDurableBrowserStorageV2(new PrivateOriginBrowserRouterV2({ provider, commons: unreachable, keystore: unreachable, identityRuntime: unreachable, identityHost: unreachable, signing: unreachable }));
  const intent = { protocol: "cord.origin.host/2", major: 2, minor: 0, registrySha256: "d17c24596fbae30c300d57ae8e51bc0c7b149ab2e91c2b9c751bedd3fbc1eeba", requestId: frame[1], productId: frame[2], operation: "storage.object.put", code: 1010, grantId: frame[4], operationId: frame[5], deadlineBlock: BigInt(frame[7]), payload: { bucketId: frame[8][0], cid: frame[8][1], length: BigInt(frame[8][2]), encrypted: frame[8][3], transferId: frame[8][4] } } as any;
  const execution = storage.start(intent); const initial: any[] = []; for await (const event of execution.events) initial.push(event); assert.equal(initial[0].kind, "accepted");
  await assert.rejects(async () => { for await (const _event of execution.resume({ kind: "provider-token", token: Uint8Array.of(1) })) void _event; }, /exact live successor/);
  await assert.rejects(async () => { for await (const _event of execution.resume({ kind: "provider-token", token })) void _event; }, /failure after durable successor send/);
  const resumedEvents: any[] = []; for await (const event of execution.resume({ kind: "provider-token", token })) resumedEvents.push(event);
  assert.equal(resumed, 2); assert.equal(resumedEvents[0].kind, "error");
  await assert.rejects(async () => { for await (const _event of execution.resume({ kind: "provider-token", token })) void _event; }, /cannot recover an exact durable continuation/);
});

test("StorageV2Execution.resume recovers an exact continuation after public storage reopen", async () => {
  const frame = decodeHostV2("RequestV2", bytes(frozen.vectors.find((candidate: any) => candidate.id === "1010-positive").wire_hex)).value as any;
  const token = bytes(protocol.vectors.find((candidate: any) => candidate.id === "provider-resume-v1").canonical_cbor_hex);
  const continuation = { operation: "storage.object.put", request: encodeHostV2("RequestV2", frame), token, predecessorOutboxId: new Uint8Array(16).fill(3), cursor: 7, hostKeyId: new Uint8Array(32).fill(4) } as const;
  let recovered = 0; let resumed = 0;
  const provider = {
    async invoke() { throw new Error("newly reopened execution must not replay the intent"); },
    recoverContinuation(operation: any, request: Uint8Array, exactToken: Uint8Array) {
      recovered += 1; assert.equal(operation, continuation.operation); assert.deepEqual(request, continuation.request); assert.deepEqual(exactToken, token); return continuation;
    },
    async resume(exact: any, _signal: any, onEvent: any) {
      resumed += 1; assert.equal(exact, continuation);
      onEvent?.(encodeHostV2Value({ 0: 2, 1: frame[1], 2: 7, 3: 3, 4: { 0: 108, 1: "REQUEST_NOT_FOUND", 2: false, 3: {} } }));
      return { error: { code: 108, name: "REQUEST_NOT_FOUND", retryable: false } };
    },
  };
  const unreachable = { async finalizedAuthority() { throw new Error("unreachable"); }, async *dispatch() { throw new Error("unreachable"); } };
  const storage = new PrivateDurableBrowserStorageV2(new PrivateOriginBrowserRouterV2({ provider, commons: unreachable, keystore: unreachable, identityRuntime: unreachable, identityHost: unreachable, signing: unreachable }));
  const intent = { protocol: "cord.origin.host/2", major: 2, minor: 0, registrySha256: "d17c24596fbae30c300d57ae8e51bc0c7b149ab2e91c2b9c751bedd3fbc1eeba", requestId: frame[1], productId: frame[2], operation: "storage.object.put", code: 1010, grantId: frame[4], operationId: frame[5], deadlineBlock: BigInt(frame[7]), payload: { bucketId: frame[8][0], cid: frame[8][1], length: BigInt(frame[8][2]), encrypted: frame[8][3], transferId: frame[8][4] } } as any;
  const reopened = storage.start(intent); const events: any[] = [];
  for await (const event of reopened.resume({ kind: "provider-token", token })) events.push(event);
  assert.equal(recovered, 1); assert.equal(resumed, 1); assert.equal(events[0].kind, "error");
});

test("real MessagePort successor verifies finalized service key and restarts exact N+1 continuation after host reopen", async () => {
  const pair = await transports(); const backend = new StrictMemoryBackend(); const random = nonceSource(); let outbox = await openOutbox(backend, pair.host, keyring(), random);
  const vector = frozen.vectors.find((candidate: any) => candidate.id === "1010-positive"); const request = bytes(vector.wire_hex);
  const frame = decodeHostV2("RequestV2", request).value as any; frame[7] = 999; const exactRequest = encodeHostV2("RequestV2", frame);
  const authority = exactCapability(pair.host, frame, Number(frame[8][2])); const token = await signedResumeToken(pair.host, pair.providerPeer.privateKey, frame, authority, 1);
  let dispatches = 0; let acked = 0; const ackWaiters: Array<() => void> = [];
  const waitAck = (target: number) => acked >= target ? Promise.resolve() : new Promise<void>((resolve) => ackWaiters.push(resolve));
  const bridge: PrivateBrowserRustProviderBridgeV2 = {
    async *dispatch(input) {
      dispatches += 1; assert.deepEqual(input.request, exactRequest);
      if (dispatches === 1) {
        assert.deepEqual(input.authority, authority);
        yield { event: accepted(frame[1]), terminalBlock: 100n, successor: { exactResumeToken: token, intendedCursor: 1 } };
        return;
      }
      assert.deepEqual(input.authority, token);
      yield { event: encodeHostV2Value({ 0: 2, 1: frame[1], 2: 1, 3: 3, 4: { 0: 108, 1: "REQUEST_NOT_FOUND", 2: false, 3: {} } }), terminalBlock: 100n };
    },
    async acknowledge(exact) { decodeHostV2("ResponseAckV1", exact); acked += 1; for (const resolve of ackWaiters.splice(0)) resolve(); return { durable: true }; },
  };
  const abort = new AbortController(); const pump = runPrivateBrowserRustProviderV2(pair.provider, bridge, { signal: abort.signal }).catch(() => undefined);
  let rotation: "current" | "retiring" = "current"; let revoked = false; let finalizedNumber = 100n; let confirmation = 0; let nextId = 0x76;
  const makeHost = () => new PrivateDurableBrowserHostV2({
    durable: new DurableBrowserHostV2(pair.host, outbox), outbox,
    finality: { async finalized() { return { number: finalizedNumber, hash: new Uint8Array(32).fill(0x42) }; } },
    authority: { async resolve() { return authority; } }, outboxIds: { next() { return new Uint8Array(16).fill(nextId++); } },
    resumeTokens: { async resolve(input) { assert.deepEqual(input.providerId, pair.host.binding.providerId); return { providerId: input.providerId, keyId: new Uint8Array(32).fill(0x91), publicKey: pair.providerPeer.peer.acknowledgementPublicKey, validFrom: 90n, validUntil: 300n, rotation, revoked, finalized: { number: input.finalized, hash: new Uint8Array(32).fill(0x42), proof: Uint8Array.of(1) } }; } },
    acknowledgements: { async confirm(input) { confirmation += 1; await waitAck(confirmation); return signConfirmation(pair.providerPeer.privateKey, outbox, input.outboxId, input.responseHash); } },
  });
  let host = makeHost(); const first = await host.invoke("storage.object.put", exactRequest); assert.deepEqual(first.continuation?.token, token); assert.equal(acked, 1);
  outbox = await openOutbox(backend, pair.host, keyring(), random); host = makeHost();
  const forged = token.slice(); forged[forged.length - 1] ^= 1;
  await assert.rejects(host.resume({ ...first.continuation!, token: forged }), /signature is invalid/);
  const cancelledToken = await signedResumeToken(pair.host, pair.providerPeer.privateKey, frame, authority, 1, 1, true);
  await assert.rejects(host.resume({ ...first.continuation!, token: cancelledToken }), /binding is invalid/);
  finalizedNumber = 228n; await assert.rejects(host.resume(first.continuation!), /not live at finalized state/); finalizedNumber = 100n; rotation = "retiring";
  await assert.rejects(host.resume(first.continuation!), /revoked, rotated, or misbound/); rotation = "current"; revoked = true;
  await assert.rejects(host.resume(first.continuation!), /revoked, rotated, or misbound/); revoked = false;
  assert.equal((await host.resume(first.continuation!)).error?.code, 108); assert.equal(dispatches, 2); assert.equal(acked, 2);
  await assert.rejects(host.resume(first.continuation!), /successor|retired|replayed|state/i);
  abort.abort(); await pump; pair.host.close(); pair.provider.close();
});

test("provider successor wire order rejects token-first and Event-without-token", async () => {
  {
    const pair = await transports(); const outbox = await openOutbox(new StrictMemoryBackend(), pair.host); const entry = await preparedEntry(pair.host);
    const durable = new DurableBrowserHostV2(pair.host, outbox); await durable.prepareAndSend({ entry }); await pair.provider.receive("RequestV2"); await pair.provider.receive("ProviderCapabilityV1");
    await pair.provider.send("ResumeTokenV1", exactResumeToken(pair.host, entry, 1));
    await assert.rejects(durable.receiveProviderEvent(100n), /before its EventV2/); pair.host.close(); pair.provider.close();
  }
  {
    const pair = await transports(); const outbox = await openOutbox(new StrictMemoryBackend(), pair.host); const entry = await preparedEntry(pair.host);
    const durable = new DurableBrowserHostV2(pair.host, outbox); await durable.prepareAndSend({ entry }); await pair.provider.receive("RequestV2"); await pair.provider.receive("ProviderCapabilityV1");
    await pair.provider.send("EventV2", accepted(entry[6])); await pair.provider.send("EventV2", progress(entry[6], 1));
    await assert.rejects(durable.receiveProviderEvent(100n), /omitted the exact successor/); pair.host.close(); pair.provider.close();
  }
});

test("StorageV2Execution.cancel durably sends CancelledEvent and waits for authenticated acknowledgement confirmation", async () => {
  const pair = await transports(); const backend = new StrictMemoryBackend(); const outbox = await openOutbox(backend, pair.host);
  const vector = frozen.vectors.find((candidate: any) => candidate.id === "1014-positive");
  const frame = decodeHostV2("RequestV2", bytes(vector.wire_hex)).value as any; const authority = exactCapability(pair.host, frame);
  let acceptedSent!: () => void; const acceptedOnPort = new Promise<void>((resolve) => { acceptedSent = resolve; });
  let ackReceived!: () => void; const acknowledged = new Promise<void>((resolve) => { ackReceived = resolve; });
  const bridge: PrivateBrowserRustProviderBridgeV2 = {
    async *dispatch(input) {
      yield { event: accepted(frame[1]), terminalBlock: 100n }; acceptedSent();
      const next = await input.cancellations[Symbol.asyncIterator]().next(); assert.equal(next.done, false);
      const exactCancel = decodeHostV2("CancelledEventV2", next.value!.event).value;
      assert.equal(Number(exactCancel[2]), 1); assert.deepEqual(next.value!.authority, authority);
      yield { event: cancelled(frame[1], 1), terminalBlock: 100n };
    },
    async acknowledge(exact) { decodeHostV2("ResponseAckV1", exact); ackReceived(); },
  };
  const abort = new AbortController(); const pump = runPrivateBrowserRustProviderV2(pair.provider, bridge, { signal: abort.signal }).catch(() => undefined);
  const provider = new PrivateDurableBrowserHostV2({
    durable: new DurableBrowserHostV2(pair.host, outbox), outbox,
    finality: { async finalized() { return { number: 100n, hash: new Uint8Array(32).fill(0x42) }; } },
    authority: { async resolve() { return authority; } }, outboxIds: { next() { return new Uint8Array(16).fill(0x75); } },
    acknowledgements: { async confirm(input) { await acknowledged; return signConfirmation(pair.providerPeer.privateKey, outbox, input.outboxId, input.responseHash); } },
  });
  const unreachable = { async finalizedAuthority() { throw new Error("unreachable"); }, async *dispatch() { throw new Error("unreachable"); } };
  const router = new PrivateOriginBrowserRouterV2({ provider, commons: unreachable, keystore: unreachable, identityRuntime: unreachable, identityHost: unreachable, signing: unreachable });
  const storage = new PrivateDurableBrowserStorageV2(router); const intent = {
    protocol: "cord.origin.host/2", major: 2, minor: 0, registrySha256: "d17c24596fbae30c300d57ae8e51bc0c7b149ab2e91c2b9c751bedd3fbc1eeba",
    requestId: frame[1], productId: frame[2], operation: "storage.object.status", code: 1014, grantId: frame[4], deadlineBlock: BigInt(frame[7]),
    payload: { bucketId: frame[8][0], cid: frame[8][1] },
  } as any;
  const execution = storage.start(intent); const iterator = execution.events[Symbol.asyncIterator](); const first = iterator.next();
  await acceptedOnPort; await execution.cancel(); assert.equal((await first).value?.kind, "accepted");
  assert.equal((await iterator.next()).value?.kind, "cancelled"); assert.equal((await iterator.next()).done, true);
  await execution.cancel(); assert.throws(() => outbox.retry(new Uint8Array(16).fill(0x75), 100n), /retired/);
  abort.abort(); await pump; pair.host.close(); pair.provider.close();
});

test("strict abort sends nothing; durable cancel restarts exactly and terminal installs before ack", async () => {
  const connected = await transports(); const backend = new StrictMemoryBackend(); const random = nonceSource(); let outbox = await openOutbox(backend, connected.host, keyring(), random);
  backend.failBeforeCommit = true; await assert.rejects(new DurableBrowserHostV2(connected.host, outbox).prepareAndSend({ entry: await preparedEntry(connected.host) }), (error) => error instanceof BrowserOutboxError && error.code === "HOST_OUTBOX_UNAVAILABLE");
  await assert.rejects(connected.provider.receive("RequestV2", { timeoutMs: 5 }), /timed out/); connected.host.close(); connected.provider.close();

  const peers = { host: await peer("restart-host"), provider: await peer("restart-provider") }; let pair = await transports(peers.host, peers.provider); outbox = await openOutbox(backend, pair.host, keyring(), random); const durable = new DurableBrowserHostV2(pair.host, outbox);
  const sent = await durable.prepareAndSend({ entry: await preparedEntry(pair.host) }); assert.deepEqual(await pair.provider.receive("RequestV2"), sent.request); await pair.provider.receive("ProviderCapabilityV1");
  await pair.provider.send("EventV2", accepted(sent.requestId)); assert.equal((await durable.receiveEvent(200n)).terminal, false);
  await pair.provider.send("EventV2", progress(sent.requestId, 1)); assert.equal((await durable.receiveEvent(200n)).terminal, false);
  backend.failAfterCommit = true; await assert.rejects(durable.prepareCancelAndSend(cancelled(sent.requestId, 2)), /strict IndexedDB commit failed/);
  await assert.rejects(pair.provider.receive("CancelledEventV2", { timeoutMs: 5 }), /timed out/); pair.host.close(); pair.provider.close();

  pair = await transports(peers.host, peers.provider); outbox = await openOutbox(backend, pair.host, keyring(), random); const restarted = new DurableBrowserHostV2(pair.host, outbox); const exact = outbox.retry(sent.outboxId, 200n); assert.equal(exact.cancel, true);
  assert.deepEqual(await restarted.resumeAndSend(sent.outboxId, 200n), exact); assert.deepEqual(await pair.provider.receive("CancelledEventV2"), exact.request); await pair.provider.receive("ProviderCapabilityV1");
  await pair.provider.send("EventV2", cancelled(sent.requestId, 2)); const terminal = await restarted.receiveEvent(200n); assert.equal(terminal.terminal, true); assert.ok(terminal.terminal); assert.deepEqual(await pair.provider.receive("ResponseAckV1"), outbox.installedAck(sent.outboxId).ack);
  pair.host.close(); pair.provider.close();
});

test("only authenticated provider confirmation retires authority and permits bounded GC", async () => {
  const peers = { host: await peer("ack-host"), provider: await peer("ack-provider") }; const pair = await transports(peers.host, peers.provider); const backend = new StrictMemoryBackend(); const random = nonceSource(); let outbox = await openOutbox(backend, pair.host, keyring(), random); const durable = new DurableBrowserHostV2(pair.host, outbox);
  const sent = await durable.prepareAndSend({ entry: await preparedEntry(pair.host) }); await pair.provider.receive("RequestV2"); await pair.provider.receive("ProviderCapabilityV1");
  await pair.provider.send("EventV2", accepted(sent.requestId)); await durable.receiveEvent(200n); await pair.provider.send("EventV2", cancelled(sent.requestId, 1)); const terminal = await durable.receiveEvent(200n); assert.ok(terminal.terminal); await pair.provider.receive("ResponseAckV1");
  const forged = new Uint8Array(64); await assert.rejects(durable.confirmAndGc({ outboxId: sent.outboxId, responseHash: terminal.responseHash, signature: forged }, 456n), /unauthenticated/);
  const signature = await signConfirmation(peers.provider.privateKey, outbox, sent.outboxId, terminal.responseHash);
  assert.equal(await durable.confirmAndGc({ outboxId: sent.outboxId, responseHash: terminal.responseHash, signature }, 455n), 0);
  const row = [...backend.records.values()][0]!; const decoder = new BrowserXChaCha20Poly1305(outbox.contextBinding, keyring(), globalThis.crypto, random); const tombstone = decodeCanonicalHostV2Value(await decoder.open(sent.outboxId, row.keyVersion, row.ciphertext)) as any;
  assert.equal(tombstone[1], 1); assert.equal(tombstone[2], undefined, "ack confirmation retained live request/authority body"); assert.throws(() => outbox.retry(sent.outboxId, 0n), /retired/);
  assert.equal(await durable.confirmAndGc({ outboxId: sent.outboxId, responseHash: terminal.responseHash, signature }, 456n), 1); assert.equal(backend.records.size, 0); pair.host.close(); pair.provider.close();
});

test("expiry creates a non-authorizing tombstone and corruption quarantines", async () => {
  const pair = await transports(); const backend = new StrictMemoryBackend(); const random = nonceSource(); let outbox = await openOutbox(backend, pair.host, keyring(), random); const prepared = await outbox.prepare({ entry: await preparedEntry(pair.host) });
  await outbox.expire(prepared.outboxId, 356n); outbox = await openOutbox(backend, pair.host, keyring(), random); assert.throws(() => outbox.retry(prepared.outboxId, 0n), /retired/); assert.equal(await outbox.gc(356n, 1), 1);
  const corruptBackend = new StrictMemoryBackend(); outbox = await openOutbox(corruptBackend, pair.host, keyring(), random); await outbox.prepare({ entry: await preparedEntry(pair.host) }); const row = [...corruptBackend.records.values()][0]!; row.ciphertext[30] ^= 1;
  await assert.rejects(openOutbox(corruptBackend, pair.host, keyring(), random), (error) => error instanceof BrowserOutboxError && error.code === "HOST_OUTBOX_CORRUPT"); assert.equal(corruptBackend.records.size, 0); assert.equal(corruptBackend.quarantine.size, 1); pair.host.close(); pair.provider.close();
});
