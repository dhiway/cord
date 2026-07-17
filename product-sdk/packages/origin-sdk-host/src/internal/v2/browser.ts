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
const DEFAULT_TIMEOUT_MS = 5_000;

export interface BrowserHostV2Peer {
  readonly source: string;
  readonly channel: string;
  readonly providerId: Uint8Array;
  readonly endpointHash: Uint8Array;
  readonly acknowledgementPublicKey: Uint8Array;
}

export interface BrowserHostV2IoOptions {
  readonly signal?: AbortSignal;
  readonly timeoutMs?: number;
}

export interface BrowserHostV2Binding {
  readonly registryHash: Uint8Array;
  readonly genesisHash: Uint8Array;
  readonly negotiatedTuple: Uint8Array;
  readonly providerId: Uint8Array;
  readonly providerEndpointHash: Uint8Array;
  readonly acknowledgementPublicKey: Uint8Array;
}

export type BrowserHostV2PeerBinding = (peer: BrowserHostV2Peer, port: MessagePort) => void | Promise<void>;
export type BrowserHostV2NegotiationSigner = (message: Uint8Array) => Uint8Array | Promise<Uint8Array>;

export class BrowserHostV2TransportError extends Error {
  readonly code:
    | "BROWSER_PEER_REJECTED"
    | "BROWSER_MESSAGE_INVALID"
    | "BROWSER_MESSAGE_TOO_LARGE"
    | "BROWSER_BACKPRESSURE"
    | "BROWSER_OPERATION_ABORTED"
    | "BROWSER_OPERATION_TIMEOUT"
    | "BROWSER_TRANSPORT_CLOSED";

  constructor(code: BrowserHostV2TransportError["code"], message: string) {
    super(message);
    this.name = "BrowserHostV2TransportError";
    this.code = code;
  }
}

interface DataEnvelope {
  readonly version: 2; readonly channel: string; readonly source: string; readonly target: string;
  readonly messageId: number; readonly kind: "data"; readonly production: HostV2TypeName;
  readonly bytes: Uint8Array;
}
interface CreditEnvelope {
  readonly version: 2; readonly channel: string; readonly source: string; readonly target: string;
  readonly messageId: number; readonly kind: "credit";
}
interface OfferEnvelope {
  readonly version: 2; readonly channel: string; readonly source: string; readonly target: string;
  readonly kind: "offer"; readonly providerId: Uint8Array; readonly endpointHash: Uint8Array;
  readonly acknowledgementPublicKey: Uint8Array; readonly offer: WireOffer; readonly signature: Uint8Array;
}
interface NegotiatedEnvelope {
  readonly version: 2; readonly channel: string; readonly source: string; readonly target: string;
  readonly kind: "negotiated"; readonly digest: Uint8Array; readonly signature: Uint8Array;
}
interface WireOffer {
  readonly protocol: string; readonly major: number; readonly minors: readonly number[];
  readonly genesis: Uint8Array; readonly finalizedSpecVersion: number;
  readonly finalizedTransactionVersion: number; readonly registrySha256: string;
  readonly features: readonly string[];
}
interface QueuedMessage { readonly production: HostV2TypeName; readonly bytes: Uint8Array }
interface PendingCredit { readonly resolve: () => void; readonly reject: (error: Error) => void; readonly cleanup: () => void }
interface PendingReceive { readonly production: HostV2TypeName; readonly resolve: (bytes: Uint8Array) => void; readonly reject: (error: Error) => void; readonly cleanup: () => void }

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
function exactBytes(value: unknown, length: number): value is Uint8Array {
  return value instanceof Uint8Array && value.length === length;
}
function equal(left: Uint8Array, right: Uint8Array): boolean {
  return left.length === right.length && left.every((byte, index) => byte === right[index]);
}
function copyPeer(peer: BrowserHostV2Peer): BrowserHostV2Peer {
  return { ...peer, providerId: peer.providerId.slice(), endpointHash: peer.endpointHash.slice(), acknowledgementPublicKey: peer.acknowledgementPublicKey.slice() };
}
function copyCanonical(production: HostV2TypeName, bytes: Uint8Array): Uint8Array {
  if (bytes.byteLength > MAX_BROWSER_HOST_V2_BYTES) return fail("BROWSER_MESSAGE_TOO_LARGE", "host-v2 browser message exceeds 4 MiB");
  const decoded = decodeHostV2(production, bytes);
  const canonical = encodeHostV2(production, decoded.value);
  if (!equal(canonical, bytes)) return fail("BROWSER_MESSAGE_INVALID", "host-v2 browser message changed on canonical decode");
  return canonical;
}
function hexBytes(value: string): Uint8Array {
  if (!/^[0-9a-f]{64}$/.test(value)) return fail("BROWSER_PEER_REJECTED", "registry SHA-256 is invalid");
  return Uint8Array.from(value.match(/../g)!.map((byte) => Number.parseInt(byte, 16)));
}
function unsignedBytes(value: number, length: number): Uint8Array {
  const output = new Uint8Array(length);
  for (let index = length - 1, remaining = value; index >= 0; index -= 1) { output[index] = remaining & 0xff; remaining = Math.floor(remaining / 256); }
  return output;
}
async function sha256(crypto: Crypto, bytes: Uint8Array): Promise<Uint8Array> {
  return new Uint8Array(await crypto.subtle.digest("SHA-256", Uint8Array.from(bytes).buffer));
}
async function verifyEd25519(crypto: Crypto, publicKey: Uint8Array, message: Uint8Array, signature: Uint8Array): Promise<boolean> {
  if (!exactBytes(signature, 64)) return false;
  try {
    const key = await crypto.subtle.importKey("raw", Uint8Array.from(publicKey).buffer, "Ed25519", false, ["verify"]);
    return crypto.subtle.verify("Ed25519", key, Uint8Array.from(signature).buffer, Uint8Array.from(message).buffer);
  } catch { return false; }
}
function concatenate(parts: readonly Uint8Array[]): Uint8Array {
  const output = new Uint8Array(parts.reduce((length, part) => length + part.length, 0));
  let offset = 0; for (const part of parts) { output.set(part, offset); offset += part.length; }
  return output;
}
function textPart(value: string): Uint8Array { const encoded = new TextEncoder().encode(value); return concatenate([unsignedBytes(encoded.length, 2), encoded]); }
function offerAuthenticationMessage(peer: BrowserHostV2Peer, offer: HostV2NegotiationOffer): Uint8Array {
  return concatenate([
    new TextEncoder().encode("cord.origin.host/2/browser-offer/v1"), textPart(peer.source), textPart(peer.channel),
    peer.providerId, peer.endpointHash, peer.acknowledgementPublicKey, textPart(offer.protocol), Uint8Array.of(offer.major),
    unsignedBytes(offer.minors.length, 2), ...offer.minors.map((minor) => unsignedBytes(minor, 2)), offer.genesis,
    unsignedBytes(offer.finalizedSpecVersion, 4), unsignedBytes(offer.finalizedTransactionVersion, 4), textPart(offer.registrySha256),
    unsignedBytes(offer.features.length, 2), ...offer.features.map(textPart),
  ]);
}
function ackAuthenticationMessage(peer: BrowserHostV2Peer, digest: Uint8Array): Uint8Array {
  return concatenate([new TextEncoder().encode("cord.origin.host/2/browser-negotiated/v1"), textPart(peer.source), textPart(peer.channel), peer.providerId, peer.endpointHash, digest]);
}
async function negotiationDigest(crypto: Crypto, negotiated: HostV2Negotiated): Promise<Uint8Array> {
  const encoder = new TextEncoder();
  const features = negotiated.features.map((feature) => encoder.encode(feature));
  return sha256(crypto, concatenate([
    encoder.encode("cord.origin.host/2/negotiated-tuple/v1"),
    Uint8Array.of(negotiated.major), unsignedBytes(negotiated.minor, 2), negotiated.genesis,
    unsignedBytes(negotiated.finalizedSpecVersion, 4), unsignedBytes(negotiated.finalizedTransactionVersion, 4),
    hexBytes(negotiated.registrySha256), unsignedBytes(features.length, 2),
    ...features.flatMap((feature) => [unsignedBytes(feature.length, 2), feature]),
  ]));
}
function validatePeerShape(peer: BrowserHostV2Peer): void {
  if (peer.source.length === 0 || peer.channel.length === 0 || !exactBytes(peer.providerId, 32)
    || !exactBytes(peer.endpointHash, 32) || !exactBytes(peer.acknowledgementPublicKey, 32)) {
    return fail("BROWSER_PEER_REJECTED", "browser peer identity is incomplete");
  }
}
function ioFailure(signal: AbortSignal | undefined, timeout: boolean): BrowserHostV2TransportError {
  return new BrowserHostV2TransportError(
    signal?.aborted ? "BROWSER_OPERATION_ABORTED" : timeout ? "BROWSER_OPERATION_TIMEOUT" : "BROWSER_TRANSPORT_CLOSED",
    signal?.aborted ? "browser host-v2 operation was aborted" : timeout ? "browser host-v2 operation timed out" : "browser host-v2 peer closed",
  );
}
function timeoutValue(value: number | undefined): number {
  const timeout = value ?? DEFAULT_TIMEOUT_MS;
  if (!Number.isSafeInteger(timeout) || timeout <= 0) return fail("BROWSER_PEER_REJECTED", "browser timeout is invalid");
  return timeout;
}

export class BrowserHostV2Transport {
  readonly #port: MessagePort;
  readonly #local: BrowserHostV2Peer;
  readonly #remote: BrowserHostV2Peer;
  readonly #negotiated: HostV2Negotiated;
  readonly #binding: BrowserHostV2Binding;
  readonly #pendingCredits = new Map<number, PendingCredit>();
  readonly #queue: QueuedMessage[] = [];
  readonly #receivers: PendingReceive[] = [];
  #nextMessageId = 0; #nextInboundMessageId = 0; #closed = false;

  private constructor(port: MessagePort, local: BrowserHostV2Peer, remote: BrowserHostV2Peer, negotiated: HostV2Negotiated, digest: Uint8Array) {
    this.#port = port; this.#local = copyPeer(local); this.#remote = copyPeer(remote); this.#negotiated = negotiated;
    this.#binding = {
      registryHash: hexBytes(negotiated.registrySha256), genesisHash: negotiated.genesis,
      negotiatedTuple: digest.slice(), providerId: remote.providerId.slice(),
      providerEndpointHash: remote.endpointHash.slice(), acknowledgementPublicKey: remote.acknowledgementPublicKey.slice(),
    };
    port.addEventListener("message", this.#onMessage); port.addEventListener("messageerror", this.#onMessageError);
    port.addEventListener("close", this.#onPeerClose); port.start();
  }

  static async connect(
    port: MessagePort, local: BrowserHostV2Peer, expectedRemote: BrowserHostV2Peer,
    bindPeer: BrowserHostV2PeerBinding, localOffer: HostV2NegotiationOffer, signNegotiation: BrowserHostV2NegotiationSigner,
    options: BrowserHostV2IoOptions & { readonly crypto?: Crypto } = {},
  ): Promise<BrowserHostV2Transport> {
    validatePeerShape(local); validatePeerShape(expectedRemote);
    if (local.channel !== expectedRemote.channel || local.source === expectedRemote.source) {
      return fail("BROWSER_PEER_REJECTED", "browser peer source or channel binding is invalid");
    }
    const crypto = options.crypto ?? globalThis.crypto;
    if (!crypto?.subtle) return fail("BROWSER_PEER_REJECTED", "browser Web Crypto is unavailable");
    if (!equal(await sha256(crypto, local.acknowledgementPublicKey), local.providerId)
      || !equal(await sha256(crypto, expectedRemote.acknowledgementPublicKey), expectedRemote.providerId)) {
      return fail("BROWSER_PEER_REJECTED", "provider ID is not bound to acknowledgement key");
    }
    let localOfferSignature: Uint8Array;
    try { localOfferSignature = Uint8Array.from(await signNegotiation(offerAuthenticationMessage(local, localOffer))); }
    catch { return fail("BROWSER_PEER_REJECTED", "browser negotiation signing failed"); }
    if (!exactBytes(localOfferSignature, 64)) return fail("BROWSER_PEER_REJECTED", "browser negotiation signature is invalid");
    const timeoutMs = timeoutValue(options.timeoutMs);
    if (options.signal?.aborted) return fail("BROWSER_OPERATION_ABORTED", "browser negotiation was aborted");
    return new Promise<BrowserHostV2Transport>((resolve, reject) => {
      let remoteOffer: HostV2NegotiationOffer | undefined; let negotiated: HostV2Negotiated | undefined;
      let digest: Uint8Array | undefined; let remoteAckDigest: Uint8Array | undefined; let settled = false; let chain = Promise.resolve();
      const finish = (error?: Error): void => {
        if (settled) return; settled = true; cleanup();
        if (error) { port.close(); reject(error); return; }
        resolve(new BrowserHostV2Transport(port, local, expectedRemote, negotiated!, digest!));
      };
      const cleanup = (): void => {
        clearTimeout(timer); options.signal?.removeEventListener("abort", onAbort);
        port.removeEventListener("message", onMessage); port.removeEventListener("messageerror", onMessageError);
        port.removeEventListener("close", onClose);
      };
      const onAbort = (): void => finish(ioFailure(options.signal, false));
      const onClose = (): void => finish(ioFailure(undefined, false));
      const onMessageError = (): void => finish(new BrowserHostV2TransportError("BROWSER_MESSAGE_INVALID", "browser negotiation structured clone failed"));
      const process = async (value: unknown): Promise<void> => {
        if (typeof value !== "object" || value === null || Array.isArray(value)) return fail("BROWSER_MESSAGE_INVALID", "browser negotiation envelope must be closed");
        const candidate = value as Record<string, unknown>;
        if (candidate.kind === "offer") {
          if (remoteOffer || !exactKeys(candidate, ["acknowledgementPublicKey", "channel", "endpointHash", "kind", "offer", "providerId", "signature", "source", "target", "version"])) return fail("BROWSER_MESSAGE_INVALID", "browser negotiation offer is open or duplicated");
          validateCommon(candidate, local, expectedRemote);
          if (!exactBytes(candidate.providerId, 32) || !equal(candidate.providerId, expectedRemote.providerId)
            || !exactBytes(candidate.endpointHash, 32) || !equal(candidate.endpointHash, expectedRemote.endpointHash)
            || !exactBytes(candidate.acknowledgementPublicKey, 32) || !equal(candidate.acknowledgementPublicKey, expectedRemote.acknowledgementPublicKey)) {
            return fail("BROWSER_PEER_REJECTED", "browser negotiation peer identity mismatched");
          }
          remoteOffer = parseOffer(candidate.offer);
          if (!exactBytes(candidate.signature, 64) || !await verifyEd25519(crypto, expectedRemote.acknowledgementPublicKey, offerAuthenticationMessage(expectedRemote, remoteOffer), candidate.signature)) return fail("BROWSER_PEER_REJECTED", "browser negotiation offer signature is unauthenticated");
          try { await bindPeer(copyPeer(expectedRemote), port); }
          catch { return fail("BROWSER_PEER_REJECTED", "browser peer binding rejected the MessagePort"); }
          negotiated = negotiateHostV2(localOffer, remoteOffer); digest = await negotiationDigest(crypto, negotiated);
          const ackSignature = Uint8Array.from(await signNegotiation(ackAuthenticationMessage(local, digest)));
          if (!exactBytes(ackSignature, 64)) return fail("BROWSER_PEER_REJECTED", "browser negotiation acknowledgement signature is invalid");
          const ack: NegotiatedEnvelope = { version: 2, channel: local.channel, source: local.source, target: expectedRemote.source, kind: "negotiated", digest: digest.slice(), signature: ackSignature };
          port.postMessage(ack);
          if (remoteAckDigest) {
            if (!equal(remoteAckDigest, digest)) return fail("BROWSER_PEER_REJECTED", "browser negotiated tuple mismatched");
            finish();
          }
          return;
        }
        if (candidate.kind === "negotiated") {
          if (remoteAckDigest || !exactKeys(candidate, ["channel", "digest", "kind", "signature", "source", "target", "version"])) return fail("BROWSER_MESSAGE_INVALID", "browser negotiation acknowledgement is open or duplicated");
          validateCommon(candidate, local, expectedRemote);
          if (!exactBytes(candidate.digest, 32) || !exactBytes(candidate.signature, 64) || !await verifyEd25519(crypto, expectedRemote.acknowledgementPublicKey, ackAuthenticationMessage(expectedRemote, candidate.digest), candidate.signature)) return fail("BROWSER_PEER_REJECTED", "browser negotiation acknowledgement is unauthenticated");
          remoteAckDigest = candidate.digest.slice();
          if (digest) { if (!equal(remoteAckDigest, digest)) return fail("BROWSER_PEER_REJECTED", "browser negotiated tuple mismatched"); finish(); }
          return;
        }
        return fail("BROWSER_MESSAGE_INVALID", "request arrived before authenticated negotiation completed");
      };
      const onMessage = (event: MessageEvent<unknown>): void => { chain = chain.then(() => process(event.data)).catch((error) => finish(error instanceof Error ? error : ioFailure(undefined, false))); };
      const timer = setTimeout(() => finish(ioFailure(undefined, true)), timeoutMs);
      options.signal?.addEventListener("abort", onAbort, { once: true });
      port.addEventListener("message", onMessage); port.addEventListener("messageerror", onMessageError); port.addEventListener("close", onClose); port.start();
      const offer: OfferEnvelope = {
        version: 2, channel: local.channel, source: local.source, target: expectedRemote.source, kind: "offer",
        providerId: local.providerId.slice(), endpointHash: local.endpointHash.slice(), acknowledgementPublicKey: local.acknowledgementPublicKey.slice(),
        offer: wireOffer(localOffer), signature: localOfferSignature,
      };
      try { port.postMessage(offer); } catch (error) { finish(error instanceof Error ? error : ioFailure(undefined, false)); }
    });
  }

  get negotiation(): HostV2Negotiated { return this.#negotiated; }
  get binding(): BrowserHostV2Binding {
    return { registryHash: this.#binding.registryHash.slice(), genesisHash: this.#binding.genesisHash.slice(), negotiatedTuple: this.#binding.negotiatedTuple.slice(), providerId: this.#binding.providerId.slice(), providerEndpointHash: this.#binding.providerEndpointHash.slice(), acknowledgementPublicKey: this.#binding.acknowledgementPublicKey.slice() };
  }

  send(production: HostV2TypeName, bytes: Uint8Array, options: BrowserHostV2IoOptions = {}): Promise<void> {
    if (this.#closed) return Promise.reject(this.#closedError());
    if (options.signal?.aborted) return Promise.reject(ioFailure(options.signal, false));
    if (this.#pendingCredits.size >= BROWSER_HOST_V2_WINDOW) return Promise.reject(new BrowserHostV2TransportError("BROWSER_BACKPRESSURE", "host-v2 browser four-message window is full"));
    let canonical: Uint8Array; try { canonical = copyCanonical(production, bytes); } catch (error) { return Promise.reject(error); }
    const messageId = this.#nextMessageId; this.#nextMessageId = (this.#nextMessageId + 1) >>> 0;
    const envelope: DataEnvelope = { version: 2, channel: this.#local.channel, source: this.#local.source, target: this.#remote.source, messageId, kind: "data", production, bytes: canonical };
    return new Promise<void>((resolve, reject) => {
      const timeout = setTimeout(() => this.#failClosed(ioFailure(undefined, true)), timeoutValue(options.timeoutMs));
      const abort = (): void => this.#failClosed(ioFailure(options.signal, false));
      const cleanup = (): void => { clearTimeout(timeout); options.signal?.removeEventListener("abort", abort); };
      this.#pendingCredits.set(messageId, { resolve, reject, cleanup }); options.signal?.addEventListener("abort", abort, { once: true });
      try { this.#port.postMessage(envelope); } catch (error) { this.#pendingCredits.delete(messageId); cleanup(); reject(error instanceof Error ? error : this.#closedError()); }
    });
  }

  receive(production: HostV2TypeName, options: BrowserHostV2IoOptions = {}): Promise<Uint8Array> {
    if (this.#closed) return Promise.reject(this.#closedError());
    if (options.signal?.aborted) return Promise.reject(ioFailure(options.signal, false));
    const index = this.#queue.findIndex((message) => message.production === production);
    if (index >= 0) return Promise.resolve(this.#queue.splice(index, 1)[0]!.bytes.slice());
    return new Promise<Uint8Array>((resolve, reject) => {
      let receiver!: PendingReceive;
      const failReceive = (error: Error): void => { const index = this.#receivers.indexOf(receiver); if (index >= 0) this.#receivers.splice(index, 1); receiver.cleanup(); reject(error); };
      const timeout = setTimeout(() => failReceive(ioFailure(undefined, true)), timeoutValue(options.timeoutMs));
      const abort = (): void => failReceive(ioFailure(options.signal, false));
      const cleanup = (): void => { clearTimeout(timeout); options.signal?.removeEventListener("abort", abort); };
      receiver = { production, resolve, reject, cleanup }; this.#receivers.push(receiver); options.signal?.addEventListener("abort", abort, { once: true });
    });
  }

  close(): void { this.#failClosed(this.#closedError()); }

  readonly #onMessage = (event: MessageEvent<unknown>): void => {
    if (this.#closed) return;
    try {
      const envelope = this.#validateEnvelope(event.data);
      if (envelope.kind === "credit") {
        const pending = this.#pendingCredits.get(envelope.messageId); if (!pending) return fail("BROWSER_MESSAGE_INVALID", "unknown browser flow credit");
        this.#pendingCredits.delete(envelope.messageId); pending.cleanup(); pending.resolve(); return;
      }
      const bytes = copyCanonical(envelope.production, envelope.bytes);
      if (envelope.messageId !== this.#nextInboundMessageId) return fail("BROWSER_MESSAGE_INVALID", "browser message sequence is duplicated or skipped");
      this.#nextInboundMessageId = (this.#nextInboundMessageId + 1) >>> 0;
      if (this.#queue.length >= BROWSER_HOST_V2_WINDOW && !this.#receivers.some((receiver) => receiver.production === envelope.production)) return fail("BROWSER_BACKPRESSURE", "browser inbound four-message window is full");
      const credit: CreditEnvelope = { version: 2, channel: this.#local.channel, source: this.#local.source, target: this.#remote.source, messageId: envelope.messageId, kind: "credit" };
      this.#port.postMessage(credit);
      const receiverIndex = this.#receivers.findIndex((receiver) => receiver.production === envelope.production);
      if (receiverIndex >= 0) { const receiver = this.#receivers.splice(receiverIndex, 1)[0]!; receiver.cleanup(); receiver.resolve(bytes.slice()); }
      else this.#queue.push({ production: envelope.production, bytes: bytes.slice() });
    } catch (error) { this.#failClosed(error instanceof Error ? error : this.#closedError()); }
  };
  readonly #onMessageError = (): void => this.#failClosed(new BrowserHostV2TransportError("BROWSER_MESSAGE_INVALID", "MessagePort structured clone failed"));
  readonly #onPeerClose = (): void => this.#failClosed(this.#closedError());

  #validateEnvelope(value: unknown): DataEnvelope | CreditEnvelope {
    if (typeof value !== "object" || value === null || Array.isArray(value)) return fail("BROWSER_MESSAGE_INVALID", "browser envelope must be a closed object");
    const candidate = value as Record<string, unknown>;
    if (candidate.kind === "credit") { if (!exactKeys(candidate, ["channel", "kind", "messageId", "source", "target", "version"])) return fail("BROWSER_MESSAGE_INVALID", "browser credit envelope is open"); }
    else if (candidate.kind === "data") { if (!exactKeys(candidate, ["bytes", "channel", "kind", "messageId", "production", "source", "target", "version"])) return fail("BROWSER_MESSAGE_INVALID", "browser data envelope is open"); }
    else return fail("BROWSER_MESSAGE_INVALID", "browser envelope kind is unknown");
    validateCommon(candidate, this.#local, this.#remote);
    if (!uint32(candidate.messageId)) return fail("BROWSER_MESSAGE_INVALID", "browser message sequence is invalid");
    if (candidate.kind === "credit") return candidate as unknown as CreditEnvelope;
    if (typeof candidate.production !== "string" || !Object.prototype.hasOwnProperty.call(HOST_V2_SCHEMAS, candidate.production) || !(candidate.bytes instanceof Uint8Array)) return fail("BROWSER_MESSAGE_INVALID", "browser production or byte clone is invalid");
    return candidate as unknown as DataEnvelope;
  }
  #failClosed(error: Error): void {
    if (this.#closed) return; this.#closed = true;
    this.#port.removeEventListener("message", this.#onMessage); this.#port.removeEventListener("messageerror", this.#onMessageError); this.#port.removeEventListener("close", this.#onPeerClose); this.#port.close();
    for (const pending of this.#pendingCredits.values()) { pending.cleanup(); pending.reject(error); } this.#pendingCredits.clear();
    for (const receiver of this.#receivers) { receiver.cleanup(); receiver.reject(error); } this.#receivers.length = 0; this.#queue.length = 0;
  }
  #closedError(): BrowserHostV2TransportError { return new BrowserHostV2TransportError("BROWSER_TRANSPORT_CLOSED", "host-v2 browser transport is closed"); }
}

function validateCommon(candidate: Record<string, unknown>, local: BrowserHostV2Peer, remote: BrowserHostV2Peer): void {
  if (candidate.version !== 2 || candidate.channel !== local.channel || candidate.source !== remote.source || candidate.target !== local.source) return fail("BROWSER_MESSAGE_INVALID", "browser source, channel, or target is misbound");
}
function wireOffer(offer: HostV2NegotiationOffer): WireOffer {
  return { protocol: offer.protocol, major: offer.major, minors: [...offer.minors], genesis: offer.genesis.slice(), finalizedSpecVersion: offer.finalizedSpecVersion, finalizedTransactionVersion: offer.finalizedTransactionVersion, registrySha256: offer.registrySha256, features: [...offer.features] };
}
function parseOffer(value: unknown): HostV2NegotiationOffer {
  if (typeof value !== "object" || value === null || Array.isArray(value) || !exactKeys(value, ["features", "finalizedSpecVersion", "finalizedTransactionVersion", "genesis", "major", "minors", "protocol", "registrySha256"])) return fail("BROWSER_MESSAGE_INVALID", "browser negotiation offer is open");
  const offer = value as Record<string, unknown>;
  if (typeof offer.protocol !== "string" || !uint32(offer.major) || !Array.isArray(offer.minors) || !offer.minors.every(uint32)
    || !exactBytes(offer.genesis, 32) || !uint32(offer.finalizedSpecVersion) || !uint32(offer.finalizedTransactionVersion)
    || typeof offer.registrySha256 !== "string" || !Array.isArray(offer.features) || !offer.features.every((feature) => typeof feature === "string")) return fail("BROWSER_MESSAGE_INVALID", "browser negotiation offer fields are invalid");
  return { protocol: offer.protocol as HostV2NegotiationOffer["protocol"], major: offer.major, minors: [...offer.minors], genesis: offer.genesis.slice(), finalizedSpecVersion: offer.finalizedSpecVersion, finalizedTransactionVersion: offer.finalizedTransactionVersion, registrySha256: offer.registrySha256, features: [...offer.features] as HostV2NegotiationOffer["features"] };
}
