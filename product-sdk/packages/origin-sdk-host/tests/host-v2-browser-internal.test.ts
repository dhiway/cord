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
import { DurableBrowserHostV2 } from "../src/internal/v2/browser-durable.ts";
import {
  BrowserHostOutboxV1, BrowserOutboxError, providerAckConfirmationMessage,
  type BrowserOutboxEncryptedRow, type StrictBrowserOutboxBackend,
} from "../src/internal/v2/browser-outbox.ts";
import { BrowserHostOutboxKeyRingV1, BrowserXChaCha20Poly1305, type BrowserHostOutboxContextV1 } from "../src/internal/v2/browser-crypto.ts";
import { BROWSER_HOST_V2_WINDOW, BrowserHostV2Transport, BrowserHostV2TransportError, type BrowserHostV2Peer } from "../src/internal/v2/browser.ts";
import { decodeCanonicalHostV2Value, decodeHostV2, encodeHostV2 } from "../src/internal/v2/codec.ts";
import {
  HOST_V2_FEATURE_IDS, HOST_V2_MAJOR, HOST_V2_MINOR, HOST_V2_PROTOCOL, HOST_V2_REGISTRY_SHA256,
  type AcceptedEventV2, type CancelledEventV2, type HostOutboxEntryV1, type ProgressEventV2,
} from "../src/internal/v2/generated.ts";
import type { HostV2NegotiationOffer } from "../src/internal/v2/session.ts";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "../../../..");
const frozen = JSON.parse(readFileSync(resolve(root, "docs/specs/origin-host-registry-v2.vectors.json"), "utf8"));
const protocol = JSON.parse(readFileSync(resolve(root, "docs/specs/protocol-executable-v2.vectors.json"), "utf8"));
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
  failBeforeCommit = false; failAfterCommit = false; strictCommits = 0;
  async load() { return [...this.records.values()].map(copyRow); }
  async putStrict(row: BrowserOutboxEncryptedRow) { if (this.failBeforeCommit) { this.failBeforeCommit = false; throw new Error("abort"); } this.records.set(row.id, copyRow(row)); this.strictCommits += 1; if (this.failAfterCommit) { this.failAfterCommit = false; throw new Error("after commit"); } }
  async deleteStrict(id: string) { this.records.delete(id); this.strictCommits += 1; }
  async quarantineStrict(row: BrowserOutboxEncryptedRow) { this.quarantine.set(row.id, copyRow(row)); this.records.delete(row.id); this.strictCommits += 1; }
}
function copyRow(row: BrowserOutboxEncryptedRow): BrowserOutboxEncryptedRow { return { id: row.id, keyVersion: row.keyVersion, ciphertext: row.ciphertext.slice() }; }
function nonceSource() { let counter = 0; return (length: number): Uint8Array => { const value = new Uint8Array(length); value.fill(0x80); value[length - 1] = counter++; return value; }; }
function keyring(active = 1, includeOld = true): BrowserHostOutboxKeyRingV1 { return new BrowserHostOutboxKeyRingV1(active, new Map([...(includeOld ? [[1, new Uint8Array(32).fill(0x8a)] as const] : []), ...(active === 2 ? [[2, new Uint8Array(32).fill(0x9a)] as const] : [])])); }
function context(transport: BrowserHostV2Transport): BrowserHostOutboxContextV1 { const binding = transport.binding; return { profileId: new Uint8Array(32).fill(0x77), registryHash: binding.registryHash, genesisHash: binding.genesisHash, negotiatedTuple: binding.negotiatedTuple, providerId: binding.providerId, providerEndpointHash: binding.providerEndpointHash }; }
async function preparedEntry(transport: BrowserHostV2Transport, version = 1): Promise<HostOutboxEntryV1> {
  const requestVector = frozen.vectors.find((vector: any) => vector.id === "1010-positive");
  const request = decodeHostV2("RequestV2", bytes(requestVector.wire_hex)).value as any;
  const requestId = new Uint8Array(16).fill(0x55); const operationId = new Uint8Array(16).fill(0x44); const grantId = new Uint8Array(32).fill(0x33);
  request[1] = requestId; request[4] = grantId; request[5] = operationId; request[7] = 0;
  const requestBytes = encodeHostV2("RequestV2", request);
  const authorityVector = protocol.vectors.find((vector: any) => vector.id === "provider-capability-v1");
  const authority = decodeHostV2("ProviderCapabilityV1", bytes(authorityVector.canonical_cbor_hex)).value as any;
  const binding = transport.binding; authority[1] = binding.registryHash; authority[2] = binding.genesisHash; authority[3] = grantId; authority[8] = binding.providerId;
  const authorityBytes = encodeHostV2("ProviderCapabilityV1", authority);
  const joined = new Uint8Array(requestBytes.length + authorityBytes.length); joined.set(requestBytes); joined.set(authorityBytes, requestBytes.length);
  const fingerprint = new Uint8Array(await globalThis.crypto.subtle.digest("SHA-256", joined));
  return { 0: 1, 1: operationId, 2: 0, 3: requestBytes, 4: authorityBytes, 5: fingerprint, 6: requestId, 7: operationId, 8: 0, 9: 0, 10: binding.registryHash, 11: binding.genesisHash, 12: binding.negotiatedTuple, 13: binding.providerId, 14: binding.providerEndpointHash, 15: 4, 17: 100, 18: 228, 19: 356, 20: version };
}
function accepted(id: Uint8Array, sequence = 0) { return encodeHostV2("AcceptedEventV2", { 0: 2, 1: id, 2: sequence, 3: 0, 4: { 0: 0 } } as AcceptedEventV2); }
function progress(id: Uint8Array, sequence: number) { return encodeHostV2("ProgressEventV2", { 0: 2, 1: id, 2: sequence, 3: 1, 4: { 0: 1 } } as ProgressEventV2); }
function cancelled(id: Uint8Array, sequence: number) { return encodeHostV2("CancelledEventV2", { 0: 2, 1: id, 2: sequence, 3: 4, 4: { 0: 107 } } as CancelledEventV2); }
async function openOutbox(backend: StrictMemoryBackend, transport: BrowserHostV2Transport, keys = keyring(), random = nonceSource()) { return BrowserHostOutboxV1.open(backend, context(transport), keys, { random }); }

async function signConfirmation(privateKey: CryptoKey, outbox: BrowserHostOutboxV1, outboxId: Uint8Array, responseHash: Uint8Array) {
  const message = providerAckConfirmationMessage(outbox.contextBinding, outboxId, responseHash);
  return new Uint8Array(await globalThis.crypto.subtle.sign("Ed25519", privateKey, message));
}

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
