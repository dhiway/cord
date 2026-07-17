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

import { decodeHostV2, encodeHostV2 } from "./codec.ts";
import { HOST_V2_SCHEMAS, type HostV2TypeName } from "./generated.ts";
import {
  negotiateHostV2,
  type HostV2Negotiated,
  type HostV2NegotiationOffer,
} from "./session.ts";

export const MAX_BROWSER_HOST_V2_BYTES = 4_194_304;
export const BROWSER_HOST_V2_WINDOW = 4;

export interface BrowserHostV2Peer {
  readonly source: string;
  readonly channel: string;
}

export type BrowserHostV2PeerBinding = (peer: BrowserHostV2Peer) => void | Promise<void>;

export class BrowserHostV2TransportError extends Error {
  readonly code:
    | "BROWSER_PEER_REJECTED"
    | "BROWSER_MESSAGE_INVALID"
    | "BROWSER_MESSAGE_TOO_LARGE"
    | "BROWSER_BACKPRESSURE"
    | "BROWSER_TRANSPORT_CLOSED";

  constructor(code: BrowserHostV2TransportError["code"], message: string) {
    super(message);
    this.name = "BrowserHostV2TransportError";
    this.code = code;
  }
}

interface DataEnvelope {
  readonly version: 2;
  readonly channel: string;
  readonly source: string;
  readonly target: string;
  readonly messageId: number;
  readonly kind: "data";
  readonly production: HostV2TypeName;
  readonly bytes: Uint8Array;
}

interface CreditEnvelope {
  readonly version: 2;
  readonly channel: string;
  readonly source: string;
  readonly target: string;
  readonly messageId: number;
  readonly kind: "credit";
}

interface QueuedMessage {
  readonly production: HostV2TypeName;
  readonly bytes: Uint8Array;
}

interface PendingCredit {
  readonly resolve: () => void;
  readonly reject: (error: Error) => void;
}

interface PendingReceive {
  readonly production: HostV2TypeName;
  readonly resolve: (bytes: Uint8Array) => void;
  readonly reject: (error: Error) => void;
}

function fail(code: BrowserHostV2TransportError["code"], message: string): never {
  throw new BrowserHostV2TransportError(code, message);
}

function exactKeys(value: object, expected: readonly string[]): boolean {
  const keys = Object.keys(value).sort();
  return keys.length === expected.length && keys.every((key, index) => key === expected[index]);
}

function uint32(value: unknown): value is number {
  return Number.isInteger(value) && Number(value) >= 0 && Number(value) <= 0xffff_ffff;
}

function copyCanonical(production: HostV2TypeName, bytes: Uint8Array): Uint8Array {
  if (bytes.byteLength > MAX_BROWSER_HOST_V2_BYTES) {
    return fail("BROWSER_MESSAGE_TOO_LARGE", "host-v2 browser message exceeds 4 MiB");
  }
  const decoded = decodeHostV2(production, bytes);
  const canonical = encodeHostV2(production, decoded.value);
  if (canonical.length !== bytes.length || canonical.some((byte, index) => byte !== bytes[index])) {
    return fail("BROWSER_MESSAGE_INVALID", "host-v2 browser message changed on canonical decode");
  }
  return canonical;
}

export class BrowserHostV2Transport {
  readonly #port: MessagePort;
  readonly #local: BrowserHostV2Peer;
  readonly #remoteSource: string;
  readonly #negotiated: HostV2Negotiated;
  readonly #pendingCredits = new Map<number, PendingCredit>();
  readonly #queue: QueuedMessage[] = [];
  readonly #receivers: PendingReceive[] = [];
  #nextMessageId = 0;
  #nextInboundMessageId = 0;
  #closed = false;

  private constructor(
    port: MessagePort,
    local: BrowserHostV2Peer,
    remoteSource: string,
    negotiated: HostV2Negotiated,
  ) {
    this.#port = port;
    this.#local = { ...local };
    this.#remoteSource = remoteSource;
    this.#negotiated = negotiated;
    port.addEventListener("message", this.#onMessage);
    port.addEventListener("messageerror", this.#onMessageError);
    port.start();
  }

  static async connect(
    port: MessagePort,
    local: BrowserHostV2Peer,
    remoteSource: string,
    bindPeer: BrowserHostV2PeerBinding,
    localOffer: HostV2NegotiationOffer,
    remoteOffer: HostV2NegotiationOffer,
  ): Promise<BrowserHostV2Transport> {
    if (local.source.length === 0 || local.channel.length === 0 || remoteSource.length === 0) {
      return fail("BROWSER_PEER_REJECTED", "browser source and channel bindings must be non-empty");
    }
    try {
      await bindPeer({ source: remoteSource, channel: local.channel });
    } catch {
      return fail("BROWSER_PEER_REJECTED", "browser peer binding rejected the MessagePort");
    }
    const negotiated = negotiateHostV2(localOffer, remoteOffer);
    return new BrowserHostV2Transport(port, local, remoteSource, negotiated);
  }

  get negotiation(): HostV2Negotiated {
    return this.#negotiated;
  }

  send(production: HostV2TypeName, bytes: Uint8Array): Promise<void> {
    if (this.#closed) return Promise.reject(this.#closedError());
    if (this.#pendingCredits.size >= BROWSER_HOST_V2_WINDOW) {
      return Promise.reject(new BrowserHostV2TransportError(
        "BROWSER_BACKPRESSURE",
        "host-v2 browser four-message window is full",
      ));
    }
    let canonical: Uint8Array;
    try {
      canonical = copyCanonical(production, bytes);
    } catch (error) {
      return Promise.reject(error);
    }
    const messageId = this.#nextMessageId;
    this.#nextMessageId = (this.#nextMessageId + 1) >>> 0;
    const envelope: DataEnvelope = {
      version: 2,
      channel: this.#local.channel,
      source: this.#local.source,
      target: this.#remoteSource,
      messageId,
      kind: "data",
      production,
      bytes: canonical,
    };
    return new Promise<void>((resolve, reject) => {
      this.#pendingCredits.set(messageId, { resolve, reject });
      try {
        this.#port.postMessage(envelope);
      } catch (error) {
        this.#pendingCredits.delete(messageId);
        reject(error instanceof Error ? error : this.#closedError());
      }
    });
  }

  receive(production: HostV2TypeName): Promise<Uint8Array> {
    if (this.#closed) return Promise.reject(this.#closedError());
    const index = this.#queue.findIndex((message) => message.production === production);
    if (index >= 0) return Promise.resolve(this.#queue.splice(index, 1)[0]!.bytes.slice());
    return new Promise<Uint8Array>((resolve, reject) => {
      this.#receivers.push({ production, resolve, reject });
    });
  }

  close(): void {
    this.#failClosed(this.#closedError());
  }

  readonly #onMessage = (event: MessageEvent<unknown>): void => {
    if (this.#closed) return;
    try {
      const envelope = this.#validateEnvelope(event.data);
      if (envelope.kind === "credit") {
        const pending = this.#pendingCredits.get(envelope.messageId);
        if (!pending) return fail("BROWSER_MESSAGE_INVALID", "unknown browser flow credit");
        this.#pendingCredits.delete(envelope.messageId);
        pending.resolve();
        return;
      }
      const bytes = copyCanonical(envelope.production, envelope.bytes);
      if (envelope.messageId !== this.#nextInboundMessageId) {
        return fail("BROWSER_MESSAGE_INVALID", "browser message sequence is duplicated or skipped");
      }
      this.#nextInboundMessageId = (this.#nextInboundMessageId + 1) >>> 0;
      if (this.#queue.length >= BROWSER_HOST_V2_WINDOW
        && !this.#receivers.some((receiver) => receiver.production === envelope.production)) {
        return fail("BROWSER_BACKPRESSURE", "browser inbound four-message window is full");
      }
      const credit: CreditEnvelope = {
        version: 2,
        channel: this.#local.channel,
        source: this.#local.source,
        target: this.#remoteSource,
        messageId: envelope.messageId,
        kind: "credit",
      };
      this.#port.postMessage(credit);
      const receiverIndex = this.#receivers.findIndex(
        (receiver) => receiver.production === envelope.production,
      );
      if (receiverIndex >= 0) {
        this.#receivers.splice(receiverIndex, 1)[0]!.resolve(bytes.slice());
      } else {
        this.#queue.push({ production: envelope.production, bytes: bytes.slice() });
      }
    } catch (error) {
      this.#failClosed(error instanceof Error ? error : this.#closedError());
    }
  };

  readonly #onMessageError = (): void => {
    this.#failClosed(new BrowserHostV2TransportError(
      "BROWSER_MESSAGE_INVALID",
      "MessagePort structured clone failed",
    ));
  };

  #validateEnvelope(value: unknown): DataEnvelope | CreditEnvelope {
    if (typeof value !== "object" || value === null || Array.isArray(value)) {
      return fail("BROWSER_MESSAGE_INVALID", "browser envelope must be a closed object");
    }
    const candidate = value as Record<string, unknown>;
    if (candidate.kind === "credit") {
      if (!exactKeys(candidate, ["channel", "kind", "messageId", "source", "target", "version"])) {
        return fail("BROWSER_MESSAGE_INVALID", "browser credit envelope is open");
      }
    } else if (candidate.kind === "data") {
      if (!exactKeys(candidate, ["bytes", "channel", "kind", "messageId", "production", "source", "target", "version"])) {
        return fail("BROWSER_MESSAGE_INVALID", "browser data envelope is open");
      }
    } else {
      return fail("BROWSER_MESSAGE_INVALID", "browser envelope kind is unknown");
    }
    if (candidate.version !== 2
      || candidate.channel !== this.#local.channel
      || candidate.source !== this.#remoteSource
      || candidate.target !== this.#local.source
      || !uint32(candidate.messageId)) {
      return fail("BROWSER_MESSAGE_INVALID", "browser source, channel, target, or sequence is misbound");
    }
    if (candidate.kind === "credit") return candidate as unknown as CreditEnvelope;
    if (typeof candidate.production !== "string"
      || !Object.prototype.hasOwnProperty.call(HOST_V2_SCHEMAS, candidate.production)
      || !(candidate.bytes instanceof Uint8Array)) {
      return fail("BROWSER_MESSAGE_INVALID", "browser production or byte clone is invalid");
    }
    return candidate as unknown as DataEnvelope;
  }

  #failClosed(error: Error): void {
    if (this.#closed) return;
    this.#closed = true;
    this.#port.removeEventListener("message", this.#onMessage);
    this.#port.removeEventListener("messageerror", this.#onMessageError);
    this.#port.close();
    for (const pending of this.#pendingCredits.values()) pending.reject(error);
    this.#pendingCredits.clear();
    for (const receiver of this.#receivers) receiver.reject(error);
    this.#receivers.length = 0;
    this.#queue.length = 0;
  }

  #closedError(): BrowserHostV2TransportError {
    return new BrowserHostV2TransportError("BROWSER_TRANSPORT_CLOSED", "host-v2 browser transport is closed");
  }
}
