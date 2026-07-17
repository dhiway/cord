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
  BrowserHostOutboxV1,
  BrowserOutboxError,
  type BrowserOutboxCrypto,
  type BrowserOutboxEncryptedRow,
  type StrictBrowserOutboxBackend,
} from "../src/internal/v2/browser-outbox.ts";
import {
  BROWSER_HOST_V2_WINDOW,
  BrowserHostV2Transport,
  BrowserHostV2TransportError,
} from "../src/internal/v2/browser.ts";
import { decodeHostV2, encodeHostV2 } from "../src/internal/v2/codec.ts";
import {
  HOST_V2_FEATURE_IDS,
  HOST_V2_MAJOR,
  HOST_V2_MINOR,
  HOST_V2_PROTOCOL,
  HOST_V2_REGISTRY_SHA256,
  type AcceptedEventV2,
  type CancelledEventV2,
  type HostOutboxEntryV1,
  type ProgressEventV2,
} from "../src/internal/v2/generated.ts";
import { HostV2SessionError, type HostV2NegotiationOffer } from "../src/internal/v2/session.ts";

const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), "../../../..");

function offer(): HostV2NegotiationOffer {
  return {
    protocol: HOST_V2_PROTOCOL,
    major: HOST_V2_MAJOR,
    minors: [HOST_V2_MINOR],
    genesis: new Uint8Array(32).fill(0x42),
    finalizedSpecVersion: 31,
    finalizedTransactionVersion: 8,
    registrySha256: HOST_V2_REGISTRY_SHA256,
    features: [...HOST_V2_FEATURE_IDS],
  };
}

function entry(): HostOutboxEntryV1 {
  const fixture = JSON.parse(readFileSync(
    resolve(repositoryRoot, "docs/specs/host-outbox-v1.vectors.json"),
    "utf8",
  )) as { base_vector: { canonical_cbor_hex: string } };
  return decodeHostV2(
    "HostOutboxEntryV1",
    Uint8Array.from(Buffer.from(fixture.base_vector.canonical_cbor_hex, "hex")),
  ).value;
}

function accepted(requestId: Uint8Array, sequence = 0): Uint8Array {
  return encodeHostV2("AcceptedEventV2", {
    0: 2, 1: requestId, 2: sequence, 3: 0, 4: { 0: 0 },
  } as AcceptedEventV2);
}

function progress(requestId: Uint8Array, sequence: number): Uint8Array {
  return encodeHostV2("ProgressEventV2", {
    0: 2, 1: requestId, 2: sequence, 3: 1, 4: { 0: 1 },
  } as ProgressEventV2);
}

function cancelled(requestId: Uint8Array, sequence: number): Uint8Array {
  return encodeHostV2("CancelledEventV2", {
    0: 2, 1: requestId, 2: sequence, 3: 4, 4: { 0: 107 },
  } as CancelledEventV2);
}

class StrictMemoryBackend implements StrictBrowserOutboxBackend {
  readonly records = new Map<string, BrowserOutboxEncryptedRow>();
  readonly quarantine = new Map<string, BrowserOutboxEncryptedRow>();
  failBeforeCommit = false;
  failAfterCommit = false;
  strictCommits = 0;

  async load(): Promise<readonly BrowserOutboxEncryptedRow[]> {
    return [...this.records.values()].map(copyRow);
  }

  async putStrict(row: BrowserOutboxEncryptedRow): Promise<void> {
    if (this.failBeforeCommit) {
      this.failBeforeCommit = false;
      throw new Error("abort before commit");
    }
    this.records.set(row.id, copyRow(row));
    this.strictCommits += 1;
    if (this.failAfterCommit) {
      this.failAfterCommit = false;
      throw new Error("crash after commit");
    }
  }

  async deleteStrict(id: string): Promise<void> {
    this.records.delete(id);
    this.strictCommits += 1;
  }

  async quarantineStrict(row: BrowserOutboxEncryptedRow): Promise<void> {
    this.quarantine.set(row.id, copyRow(row));
    this.records.delete(row.id);
    this.strictCommits += 1;
  }
}

function copyRow(row: BrowserOutboxEncryptedRow): BrowserOutboxEncryptedRow {
  return { id: row.id, keyVersion: row.keyVersion, ciphertext: row.ciphertext.slice() };
}

class AuthenticatedTestCrypto implements BrowserOutboxCrypto {
  async seal(_id: Uint8Array, plaintext: Uint8Array) {
    let tag = 0;
    const ciphertext = new Uint8Array(plaintext.length + 1);
    plaintext.forEach((byte, index) => {
      tag = (tag + byte + index) & 0xff;
      ciphertext[index] = byte ^ 0xa5;
    });
    ciphertext[plaintext.length] = tag;
    return { keyVersion: 1, ciphertext };
  }

  async open(_id: Uint8Array, keyVersion: number, ciphertext: Uint8Array): Promise<Uint8Array> {
    if (keyVersion !== 1 || ciphertext.length === 0) throw new Error("wrong key");
    const plaintext = ciphertext.slice(0, -1).map((byte) => byte ^ 0xa5);
    let tag = 0;
    plaintext.forEach((byte, index) => { tag = (tag + byte + index) & 0xff; });
    if (ciphertext.at(-1) !== tag) throw new Error("authentication failed");
    return plaintext;
  }

  async digest(bytes: Uint8Array): Promise<Uint8Array> {
    const digest = new Uint8Array(32);
    bytes.forEach((byte, index) => { digest[index % 32] = (digest[index % 32]! + byte + index) & 0xff; });
    return digest;
  }
}

async function transports(): Promise<[BrowserHostV2Transport, BrowserHostV2Transport]> {
  const channel = new MessageChannel();
  return Promise.all([
    BrowserHostV2Transport.connect(
      channel.port1,
      { source: "host", channel: "festival" },
      "worker",
      (peer) => assert.deepEqual(peer, { source: "worker", channel: "festival" }),
      offer(),
      offer(),
    ),
    BrowserHostV2Transport.connect(
      channel.port2,
      { source: "worker", channel: "festival" },
      "host",
      (peer) => assert.deepEqual(peer, { source: "host", channel: "festival" }),
      offer(),
      offer(),
    ),
  ]);
}

test("real MessageChannel enforces source binding, canonical clone decode, cap, and four-message window", async () => {
  const channel = new MessageChannel();
  const host = await BrowserHostV2Transport.connect(
    channel.port1,
    { source: "host", channel: "festival" },
    "worker",
    () => {},
    offer(),
    offer(),
  );
  const canonical = encodeHostV2("RequestId", new Uint8Array(16));
  await assert.rejects(
    host.send("Bytes4MiB", new Uint8Array(4_194_305)),
    (error) => error instanceof BrowserHostV2TransportError && error.code === "BROWSER_MESSAGE_TOO_LARGE",
  );
  const pending = Array.from({ length: BROWSER_HOST_V2_WINDOW }, () => host.send("RequestId", canonical));
  await assert.rejects(
    host.send("RequestId", canonical),
    (error) => error instanceof BrowserHostV2TransportError && error.code === "BROWSER_BACKPRESSURE",
  );
  host.close();
  await Promise.allSettled(pending);

  const [boundHost, worker] = await transports();
  const receive = boundHost.receive("RequestId");
  channel.port2.close();
  worker.close();
  boundHost.close();
  await assert.rejects(receive, /closed/);

  const hostile = new MessageChannel();
  const victim = await BrowserHostV2Transport.connect(
    hostile.port1,
    { source: "host", channel: "festival" },
    "worker",
    () => {},
    offer(),
    offer(),
  );
  const rejected = victim.receive("RequestId");
  hostile.port2.postMessage({
    version: 2,
    channel: "festival",
    source: "attacker",
    target: "host",
    messageId: 0,
    kind: "data",
    production: "RequestId",
    bytes: canonical,
  });
  await assert.rejects(rejected, /misbound/);
  hostile.port2.close();
});

test("strict commit is awaited before postMessage and durable terminal precedes ack across restart", async () => {
  const backend = new StrictMemoryBackend();
  const crypto = new AuthenticatedTestCrypto();
  let outbox = await BrowserHostOutboxV1.open(backend, crypto);
  const channel = new MessageChannel();
  let posts = 0;
  channel.port2.onmessage = () => { posts += 1; };
  const host = await BrowserHostV2Transport.connect(
    channel.port1,
    { source: "host", channel: "festival" },
    "worker",
    () => {},
    offer(),
    offer(),
  );
  backend.failBeforeCommit = true;
  await assert.rejects(
    new DurableBrowserHostV2(host, outbox).prepareAndSend({ entry: entry() }),
    (error) => error instanceof BrowserOutboxError && error.code === "HOST_OUTBOX_UNAVAILABLE",
  );
  await new Promise((resolve) => setTimeout(resolve, 20));
  assert.equal(posts, 0, "aborted strict transaction leaked a MessagePort request");
  channel.port2.close();

  [outbox] = [await BrowserHostOutboxV1.open(backend, crypto)];
  const [hostTransport, provider] = await transports();
  const durable = new DurableBrowserHostV2(hostTransport, outbox);
  const prepared = await durable.prepareAndSend({ entry: entry() });
  assert.deepEqual(await provider.receive("RequestV2"), prepared.request);
  assert.deepEqual(await provider.receive("ProviderCapabilityV1"), prepared.authority);
  await provider.send("EventV2", accepted(prepared.requestId));
  assert.equal((await durable.receiveEvent(200n)).terminal, false);
  await provider.send("EventV2", progress(prepared.requestId, 1));
  assert.equal((await durable.receiveEvent(200n)).terminal, false);

  backend.failAfterCommit = true;
  await provider.send("EventV2", cancelled(prepared.requestId, 2));
  await assert.rejects(
    durable.receiveEvent(200n),
    (error) => error instanceof BrowserOutboxError && error.code === "HOST_OUTBOX_UNAVAILABLE",
  );
  provider.close();
  outbox = await BrowserHostOutboxV1.open(backend, crypto);
  const installed = outbox.installedAck(prepared.outboxId);

  const [restartHost, restartProvider] = await transports();
  const restarted = new DurableBrowserHostV2(restartHost, outbox);
  const ackReceive = restartProvider.receive("ResponseAckV1");
  assert.deepEqual(await restarted.resumeAck(prepared.outboxId), installed.responseHash);
  assert.deepEqual(await ackReceive, installed.ack);
  assert.equal(await restarted.confirmAndGc(prepared.outboxId, installed.responseHash, 455n), 0);
  assert.equal(await restarted.confirmAndGc(prepared.outboxId, installed.responseHash, 456n), 1);
  restartHost.close();
  restartProvider.close();
  assert.ok(backend.strictCommits >= 5);
});

test("browser outbox bounds TTL and authenticated corruption fail closed", async () => {
  const crypto = new AuthenticatedTestCrypto();
  const fullBackend = new StrictMemoryBackend();
  const full = await BrowserHostOutboxV1.open(fullBackend, crypto, { bytes: 1 });
  await assert.rejects(
    full.prepare({ entry: entry() }),
    (error) => error instanceof BrowserOutboxError && error.code === "HOST_OUTBOX_FULL",
  );

  const backend = new StrictMemoryBackend();
  const outbox = await BrowserHostOutboxV1.open(backend, crypto);
  const prepared = await outbox.prepare({ entry: entry() });
  assert.throws(
    () => outbox.retry(prepared.outboxId, BigInt(entry()[19])),
    (error) => error instanceof BrowserOutboxError && error.code === "HOST_OUTBOX_EXPIRED",
  );
  await outbox.expire(prepared.outboxId, BigInt(entry()[19]));
  const reopenedExpired = await BrowserHostOutboxV1.open(backend, crypto);
  assert.throws(
    () => reopenedExpired.retry(prepared.outboxId, 0n),
    (error) => error instanceof BrowserOutboxError && error.code === "HOST_OUTBOX_STATE_INVALID",
  );
  const row = [...backend.records.values()][0]!;
  row.ciphertext[0] ^= 1;
  await assert.rejects(
    BrowserHostOutboxV1.open(backend, crypto),
    (error) => error instanceof BrowserOutboxError && error.code === "HOST_OUTBOX_CORRUPT",
  );
  assert.equal(backend.records.size, 0);
  assert.equal(backend.quarantine.size, 1);
});

test("post-commit crash resumes exact request and normal progress/cancel closes the session", async () => {
  const backend = new StrictMemoryBackend();
  const crypto = new AuthenticatedTestCrypto();
  let outbox = await BrowserHostOutboxV1.open(backend, crypto);
  const channel = new MessageChannel();
  let posts = 0;
  channel.port2.onmessage = () => { posts += 1; };
  const transport = await BrowserHostV2Transport.connect(
    channel.port1,
    { source: "host", channel: "festival" },
    "worker",
    () => {},
    offer(),
    offer(),
  );
  backend.failAfterCommit = true;
  await assert.rejects(
    new DurableBrowserHostV2(transport, outbox).prepareAndSend({ entry: entry() }),
    (error) => error instanceof BrowserOutboxError && error.code === "HOST_OUTBOX_UNAVAILABLE",
  );
  await new Promise((resolve) => setTimeout(resolve, 20));
  assert.equal(posts, 0, "durable post-commit crash sent before transaction completion resolved");
  transport.close();
  channel.port2.close();

  outbox = await BrowserHostOutboxV1.open(backend, crypto);
  const frozen = entry();
  const outboxId = frozen[1];
  const exact = outbox.retry(outboxId, 200n);
  const [host, provider] = await transports();
  const durable = new DurableBrowserHostV2(host, outbox);
  assert.deepEqual(await durable.resumeAndSend(outboxId, 200n), exact);
  assert.deepEqual(await provider.receive("RequestV2"), exact.request);
  assert.deepEqual(await provider.receive("ProviderCapabilityV1"), exact.authority);

  await provider.send("EventV2", accepted(exact.requestId, 0));
  assert.equal((await durable.receiveEvent(200n)).terminal, false);
  await provider.send("EventV2", progress(exact.requestId, 1));
  assert.equal((await durable.receiveEvent(200n)).terminal, false);
  await provider.send("EventV2", cancelled(exact.requestId, 2));
  const terminal = await durable.receiveEvent(200n);
  assert.equal(terminal.terminal, true);
  assert.ok(terminal.terminal);
  assert.deepEqual(await provider.receive("ResponseAckV1"), outbox.installedAck(outboxId).ack);

  await provider.send("EventV2", progress(exact.requestId, 3));
  await assert.rejects(
    durable.receiveEvent(200n),
    (error) => error instanceof HostV2SessionError,
  );
  host.close();
  provider.close();
});
