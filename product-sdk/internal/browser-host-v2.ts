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

/**
 * Private P3 browser binding for the frozen Host-v2 registry. Nothing in this file is exported
 * from a package entrypoint: P4/P5 own the public cutover after the Rust dispatcher is connected.
 */

import { encodeStorageV2Intent } from "../packages/origin-sdk-cloud-storage/src/internal/storage-v2-codec.ts";
import { blake2b256 } from "../packages/origin-sdk-crypto/src/index.ts";
import {
  type StorageV2Event, type StorageV2Execution, type StorageV2Intent, type StorageV2Operation,
  type StorageV2Resume, type StorageV2Transport,
} from "../packages/origin-sdk-cloud-storage/src/internal/storage-v2-intents.ts";
import {
  type IdentityInvocationV2, type IdentityV2Bridge, type IdentityV2BridgeResult,
  type IdentityV2Call,
} from "../packages/origin-sdk-identity/src/v2.ts";
import { BrowserHostOutboxV1 } from "../packages/origin-sdk-host/src/internal/v2/browser-outbox.ts";
import { DurableBrowserHostV2 } from "../packages/origin-sdk-host/src/internal/v2/browser-durable.ts";
import { BrowserHostV2Transport, type BrowserHostV2IoOptions } from "../packages/origin-sdk-host/src/internal/v2/browser.ts";
import { decodeHostV2, encodeHostV2, type HostV2Map } from "../packages/origin-sdk-host/src/internal/v2/codec.ts";
import {
  HOST_V2_ERROR_BINDINGS, HOST_V2_OPERATION_BINDINGS,
  type HostOutboxEntryV1, type HostV2TypeName,
} from "../packages/origin-sdk-host/src/internal/v2/generated.ts";
import type { PrivateStorageExecutorV2, PrivateStorageIntentV2, PrivateStorageUploadV2, StorageOperationV2 } from "../packages/origin-sdk-apps/src/internal/v2-publishing.ts";

type Operation = keyof typeof HOST_V2_OPERATION_BINDINGS;
type WireMap = Record<number, unknown>;
export type PrivateProviderByteOperationV2 = "storage.object.put" | "storage.object.get" | "storage.object.range" | "storage.object.status";
export type PrivateCommonsOperationV2 = Exclude<StorageV2Operation, PrivateProviderByteOperationV2 | "storage.keys.export" | "storage.keys.import">;
export type PrivateKeystoreOperationV2 = "storage.keys.export" | "storage.keys.import";
export type PrivateFinalizedIdentityOperationV2 = "identity.account" | "identity.humanity.status" | "identity.entitlements.read";
export type PrivateHostIdentityOperationV2 = "identity.profile.read" | "identity.profile.disclose" | "identity.humanity.prove" | "identity.subject.derive";
export type PrivateSigningOperationV2 = "transaction.sign";

const PROVIDER_BYTE_OPERATIONS: ReadonlySet<Operation> = new Set<Operation>([
  "storage.object.put", "storage.object.get", "storage.object.range", "storage.object.status",
]);

export interface PrivateBrowserFinalityResolverV2 {
  finalized(signal?: AbortSignal): Promise<{ readonly number: bigint; readonly hash: Uint8Array }>;
}

/** Capability material is selected by the trusted host. Raw signing/encryption keys never cross this interface. */
export interface PrivateBrowserAuthorityResolverV2 {
  resolve(input: {
    readonly operation: PrivateProviderByteOperationV2; readonly code: number; readonly productId: string;
    readonly requestId: Uint8Array; readonly grantId?: Uint8Array; readonly operationId?: Uint8Array;
    readonly finalized: bigint;
  }, signal?: AbortSignal): Promise<Uint8Array>;
}

export interface PrivateBrowserOutboxIdResolverV2 { next(): Uint8Array }
export interface PrivateBrowserAckConfirmationResolverV2 {
  confirm(input: { readonly outboxId: Uint8Array; readonly responseHash: Uint8Array }, signal?: AbortSignal): Promise<Uint8Array>;
}

interface PrivateBrowserInvocationControlV2 {
  bind(cancel: () => Promise<void>): Promise<void>;
  finish(error?: unknown): void;
}

/** Exact-byte process boundary for the Rust P1/P2 dispatcher; it deliberately defines no duplicate DTO. */
export interface PrivateBrowserRustProviderBridgeV2 {
  dispatch(input: {
    readonly request: Uint8Array; readonly authority: Uint8Array;
    readonly upload?: AsyncIterable<Uint8Array>;
    readonly cancellations: AsyncIterable<{ readonly event: Uint8Array; readonly authority: Uint8Array }>;
    readonly signal?: AbortSignal;
  }): AsyncIterable<{ readonly event: Uint8Array; readonly terminalBlock: bigint }>;
  acknowledge(exactAck: Uint8Array, signal?: AbortSignal): Promise<void>;
}

interface PrivateExactHostExecutionV2 { readonly event: Uint8Array; readonly terminalBlock: bigint }
export interface PrivateFinalizedHostAuthorityV2 { readonly number: bigint; readonly hash: Uint8Array; readonly proof: Uint8Array }
interface PrivateExactHostBridgeV2<Op extends Operation> {
  finalizedAuthority(input: {
    readonly operation: Op; readonly code: number; readonly productId: string; readonly requestId: Uint8Array;
  }, signal?: AbortSignal): Promise<PrivateFinalizedHostAuthorityV2>;
  dispatch(input: {
    readonly operation: Op; readonly request: Uint8Array; readonly authority: PrivateFinalizedHostAuthorityV2;
    readonly signal?: AbortSignal;
  }): AsyncIterable<PrivateExactHostExecutionV2>;
}
export interface PrivateCommonsRuntimeBridgeV2 extends PrivateExactHostBridgeV2<PrivateCommonsOperationV2> {}
export interface PrivateHostKeystoreBridgeV2 extends PrivateExactHostBridgeV2<PrivateKeystoreOperationV2> {}
export interface PrivateFinalizedIdentityRuntimeBridgeV2 extends PrivateExactHostBridgeV2<PrivateFinalizedIdentityOperationV2> {}
export interface PrivateHostIdentityAuthorityBridgeV2 extends PrivateExactHostBridgeV2<PrivateHostIdentityOperationV2> {}
export interface PrivateHostSigningAuthorityBridgeV2 extends PrivateExactHostBridgeV2<PrivateSigningOperationV2> {}
export interface PrivateProviderByteBridgeV2 {
  invoke(
    operation: PrivateProviderByteOperationV2, exactRequest: Uint8Array, upload?: PrivateStorageUploadV2,
    signal?: AbortSignal, onEvent?: (event: Uint8Array) => void, control?: PrivateBrowserInvocationControlV2,
  ): Promise<PrivateInvocationResultV2>;
}

export interface PrivateInvocationResultV2 {
  readonly value?: unknown;
  readonly error?: PrivateBrowserErrorV2;
}
interface PrivateBrowserErrorV2 {
  readonly code: number; readonly name: string; readonly retryable: boolean;
  readonly details?: { readonly message?: string; readonly lower?: bigint; readonly upper?: bigint; readonly hash?: Uint8Array };
}

export class PrivateDurableBrowserHostV2 implements PrivateProviderByteBridgeV2 {
  readonly #durable: DurableBrowserHostV2;
  readonly #outbox: BrowserHostOutboxV1;
  readonly #finality: PrivateBrowserFinalityResolverV2;
  readonly #authority: PrivateBrowserAuthorityResolverV2;
  readonly #ids: PrivateBrowserOutboxIdResolverV2;
  readonly #acknowledgements?: PrivateBrowserAckConfirmationResolverV2;

  constructor(input: {
    readonly durable: DurableBrowserHostV2; readonly outbox: BrowserHostOutboxV1;
    readonly finality: PrivateBrowserFinalityResolverV2; readonly authority: PrivateBrowserAuthorityResolverV2;
    readonly outboxIds: PrivateBrowserOutboxIdResolverV2; readonly acknowledgements?: PrivateBrowserAckConfirmationResolverV2;
  }) {
    this.#durable = input.durable; this.#outbox = input.outbox; this.#finality = input.finality;
    this.#authority = input.authority; this.#ids = input.outboxIds; this.#acknowledgements = input.acknowledgements;
  }

  async invoke(
    operation: PrivateProviderByteOperationV2, exactRequest: Uint8Array, upload?: PrivateStorageUploadV2,
    signal?: AbortSignal, onEvent?: (event: Uint8Array) => void, control?: PrivateBrowserInvocationControlV2,
  ): Promise<PrivateInvocationResultV2> {
    const binding = HOST_V2_OPERATION_BINDINGS[operation];
    if (!binding) throw new TypeError("Host-v2 operation is not in the generated registry");
    const request = decodeHostV2(binding.frame as HostV2TypeName, exactRequest).value as WireMap;
    if (Number(request[3]) !== binding.code) throw new TypeError("Host-v2 request code mismatches its generated binding");
    const finalized = await this.#finality.finalized(signal);
    const authority = await this.#authority.resolve({
      operation, code: binding.code, productId: request[2] as string,
      requestId: bytes(request[1], 16, "request ID"),
      ...(request[4] instanceof Uint8Array ? { grantId: request[4].slice() } : {}),
      ...(request[5] instanceof Uint8Array ? { operationId: request[5].slice() } : {}),
      finalized: finalized.number,
    }, signal);
    const capability = decodeHostV2("ProviderCapabilityV1", authority).value;
    const context = this.#outbox.contextBinding;
    const operationId = request[5] instanceof Uint8Array ? request[5].slice() : new Uint8Array(16);
    const fingerprint = await this.#outbox.digest(concat(exactRequest, authority));
    const outboxId = bytes(this.#ids.next(), 16, "outbox ID");
    const entry: HostOutboxEntryV1 = {
      0: 1, 1: outboxId, 2: 0, 3: exactRequest.slice(), 4: authority.slice(), 5: fingerprint,
      6: bytes(request[1], 16, "request ID"), 7: operationId, 8: 0n, 9: 0,
      10: context.registryHash, 11: context.genesisHash, 12: context.negotiatedTuple,
      13: context.providerId, 14: context.providerEndpointHash, 15: 2,
      17: finalized.number, 18: BigInt(capability[13]), 19: finalized.number + 256n,
      20: this.#outbox.activeKeyVersion,
    };
    await this.#durable.prepareAndSend({ entry }, io(signal));
    const uploadWindow = upload ? new ProviderUploadAckWindowV2(signal) : undefined;
    const uploadSending = upload ? this.#streamUpload(operationId, upload, uploadWindow!, signal) : undefined;
    if (uploadSending) void uploadSending.catch(() => undefined);
    let cancelRequested = false; let cancelSent: Promise<void> | undefined;
    const requestCancel = (): Promise<void> => {
      if (cancelSent) return cancelSent;
      cancelRequested = true; uploadWindow?.seal();
      const exact = encodeHostV2("CancelledEventV2", { 0: 2, 1: entry[6], 2: nextSequence, 3: 4, 4: { 0: 107 } });
      cancelSent = this.#durable.prepareCancelAndSend(exact, io(signal)).then(() => undefined); return cancelSent;
    };
    let nextSequence = 0;
    while (true) {
      const received = await this.#durable.receiveEvent(finalized.number, io(signal));
      const event = decodeHostV2("EventV2", received.event).value as WireMap;
      nextSequence = Number(event[2]) + 1;
      if (Number(event[3]) === 0 && control) await control.bind(requestCancel);
      if (cancelRequested && Number(event[3]) !== 4) throw new TypeError("cancelled provider operation emitted a later progress or effect");
      onEvent?.(received.event.slice());
      if (Number(event[3]) === 1 && uploadWindow) {
        const chunksAcked = (event[4] as WireMap)[2];
        if (chunksAcked !== undefined) uploadWindow.advance(Number(chunksAcked));
      }
      if (!received.terminal) continue;
      uploadWindow?.seal();
      if (cancelRequested && Number(event[3]) !== 4) throw new TypeError("provider cancellation did not terminate as CancelledEventV2");
      if (cancelRequested && !this.#acknowledgements) throw new TypeError("durable provider cancellation requires authenticated acknowledgement confirmation");
      if (this.#acknowledgements) {
        const signature = await this.#acknowledgements.confirm({ outboxId, responseHash: received.responseHash }, signal);
        await this.#durable.confirmAndGc({ outboxId, responseHash: received.responseHash, signature }, finalized.number);
      }
      if (Number(event[3]) === 4) return {};
      if (event[3] === 3) {
        if (uploadSending) void uploadSending.catch(() => undefined);
        return { error: decodeError(event[4] as WireMap) };
      }
      uploadWindow?.complete(); if (uploadSending) await uploadSending;
      return { value: decodeResult(operation, event[4] as WireMap) };
    }
  }

  async #streamUpload(operationId: Uint8Array, upload: PrivateStorageUploadV2, window: ProviderUploadAckWindowV2, signal?: AbortSignal): Promise<void> {
    let sequence = 0; let length = 0n;
    for await (const source of upload.bytes) {
      if (signal?.aborted) throw signal.reason ?? new Error("upload aborted");
      for (let offset = 0; offset < source.length; offset += 262_144) {
        const chunk = source.slice(offset, offset + 262_144); length += BigInt(chunk.length);
        await window.beforeSend(sequence); const digest = blake2b256(chunk);
        const exact = encodeHostV2("ProviderTransferChunkV1", { 0: 1, 1: operationId, 2: sequence++, 3: chunk, 4: digest });
        window.sent(sequence); await this.#durable.sendProviderTransferChunk(exact, io(signal));
      }
    }
    if (length !== upload.length) throw new TypeError("one-shot upload length mismatches its object.put intent");
  }
}

class ProviderUploadAckWindowV2 {
  #sent = 0; #acked = 0; #sealed = false; #waiters = new Set<() => void>();
  readonly #signal?: AbortSignal; readonly #onAbort: () => void;
  constructor(signal?: AbortSignal) {
    this.#signal = signal; this.#onAbort = () => this.seal();
    if (signal?.aborted) this.#sealed = true;
    else signal?.addEventListener("abort", this.#onAbort, { once: true });
  }
  async beforeSend(sequence: number): Promise<void> {
    while (!this.#sealed && sequence - this.#acked >= 4) await new Promise<void>((resolve) => this.#waiters.add(resolve));
    if (this.#sealed) throw new TypeError("provider upload terminated before progress acknowledgement");
  }
  sent(count: number): void {
    if (this.#sealed) throw new TypeError("provider upload attempted a post-terminal send");
    if (count !== this.#sent + 1) throw new TypeError("provider upload send cursor is non-contiguous"); this.#sent = count;
  }
  advance(count: number): void {
    if (!Number.isSafeInteger(count) || count < this.#acked || count > this.#sent) throw new TypeError("provider chunks_acked cursor is invalid");
    this.#acked = count; this.#wake();
  }
  complete(): void { if (this.#acked !== this.#sent) throw new TypeError("provider terminal result preceded upload acknowledgement"); }
  seal(): void { if (this.#sealed) return; this.#sealed = true; this.#signal?.removeEventListener("abort", this.#onAbort); this.#wake(); }
  #wake(): void { for (const resolve of this.#waiters) resolve(); this.#waiters.clear(); }
}

/** Exhaustive authority router. Only the four provider-byte operations can reach the durable provider port. */
export class PrivateOriginBrowserRouterV2 {
  readonly #provider: PrivateProviderByteBridgeV2; readonly #commons: PrivateCommonsRuntimeBridgeV2;
  readonly #keystore: PrivateHostKeystoreBridgeV2; readonly #identityRuntime: PrivateFinalizedIdentityRuntimeBridgeV2;
  readonly #identityHost: PrivateHostIdentityAuthorityBridgeV2; readonly #signing: PrivateHostSigningAuthorityBridgeV2;
  constructor(input: {
    readonly provider: PrivateProviderByteBridgeV2; readonly commons: PrivateCommonsRuntimeBridgeV2;
    readonly keystore: PrivateHostKeystoreBridgeV2; readonly identityRuntime: PrivateFinalizedIdentityRuntimeBridgeV2;
    readonly identityHost: PrivateHostIdentityAuthorityBridgeV2; readonly signing: PrivateHostSigningAuthorityBridgeV2;
  }) {
    this.#provider = input.provider; this.#commons = input.commons; this.#keystore = input.keystore;
    this.#identityRuntime = input.identityRuntime; this.#identityHost = input.identityHost; this.#signing = input.signing;
  }
  async invoke(
    operation: Operation, exactRequest: Uint8Array, upload?: PrivateStorageUploadV2,
    signal?: AbortSignal, onEvent?: (event: Uint8Array) => void, control?: PrivateBrowserInvocationControlV2,
  ): Promise<PrivateInvocationResultV2> {
    switch (operation) {
      case "storage.object.put": case "storage.object.get": case "storage.object.range": case "storage.object.status":
        return this.#provider.invoke(operation, exactRequest, upload, signal, onEvent, control);
      case "storage.bucket.create": case "storage.bucket.get": case "storage.bucket.grant": case "storage.bucket.revoke":
      case "storage.object.delete": case "storage.checkpoint.status": case "storage.checkpoint.subscribe":
      case "storage.replica.status": case "storage.replica.subscribe": case "storage.deletion.status":
      case "storage.deletion.subscribe": case "storage.drive.read": case "storage.drive.commit": case "storage.drive.share":
      case "storage.s3.put": case "storage.s3.get": case "storage.s3.list": case "storage.s3.delete":
      case "storage.publish": case "storage.resolve":
        return invokeExactHost(this.#commons, operation, exactRequest, signal, onEvent);
      case "storage.keys.export": case "storage.keys.import":
        return invokeExactHost(this.#keystore, operation, exactRequest, signal, onEvent);
      case "identity.account": case "identity.humanity.status": case "identity.entitlements.read":
        return invokeExactHost(this.#identityRuntime, operation, exactRequest, signal, onEvent);
      case "identity.profile.read": case "identity.profile.disclose": case "identity.humanity.prove": case "identity.subject.derive":
        return invokeExactHost(this.#identityHost, operation, exactRequest, signal, onEvent);
      case "transaction.sign": return invokeExactHost(this.#signing, operation, exactRequest, signal, onEvent);
    }
    const exhaustive: never = operation; throw new TypeError(`unreachable Host-v2 operation ${exhaustive}`);
  }
}

export class PrivateDurableBrowserStorageV2 implements StorageV2Transport, PrivateStorageExecutorV2 {
  readonly #host: PrivateOriginBrowserRouterV2;
  constructor(host: PrivateOriginBrowserRouterV2) { this.#host = host; }

  start<Op extends StorageV2Operation>(intent: StorageV2Intent<Op>): StorageV2Execution<Op> {
    const control = new StorageInvocationControlV2();
    const events = this.#events(intent, undefined, control);
    return { events, cancel: () => control.cancel(), resume: (resume: StorageV2Resume) => failClosedResumeV2(resume) };
  }

  async execute<Op extends StorageOperationV2>(
    intent: PrivateStorageIntentV2<Op>, upload: Op extends "storage.object.put" ? PrivateStorageUploadV2 : undefined,
    signal?: AbortSignal,
  ): Promise<unknown> {
    const result = await this.#host.invoke(intent.operation, encodePrivateIntent(intent), upload, signal);
    if (result.error) throw Object.assign(new Error(result.error.name), result.error);
    return result.value;
  }

  async *#events<Op extends StorageV2Operation>(intent: StorageV2Intent<Op>, upload: PrivateStorageUploadV2 | undefined, control: StorageInvocationControlV2): AsyncIterable<StorageV2Event<Op>> {
    const buffered: StorageV2Event<Op>[] = [];
    let result: PrivateInvocationResultV2;
    try {
      result = await this.#host.invoke(intent.operation, encodeStorageV2Intent(intent), upload, undefined, (exact) => {
        buffered.push(decodeStorageEvent(intent.operation, exact) as StorageV2Event<Op>);
      }, control);
      control.finish();
    } catch (error) { control.finish(error); throw error; }
    for (const event of buffered) yield event;
    if (result.error) return;
  }
}

class StorageInvocationControlV2 implements PrivateBrowserInvocationControlV2 {
  #cancel?: () => Promise<void>; #requested = false; #finished = false; #error: unknown;
  #cancelSent?: Promise<void>; readonly #completion: Promise<void>; #resolve!: () => void; #reject!: (error: unknown) => void;
  constructor() { this.#completion = new Promise<void>((resolve, reject) => { this.#resolve = resolve; this.#reject = reject; }); }
  async bind(cancel: () => Promise<void>): Promise<void> {
    if (this.#cancel || this.#finished) throw new TypeError("durable provider cancellation binding is not live");
    this.#cancel = cancel; if (this.#requested) await this.#send();
  }
  async cancel(): Promise<void> {
    if (this.#finished) { if (this.#error !== undefined) throw this.#error; return; }
    this.#requested = true; if (this.#cancel) await this.#send(); await this.#completion;
  }
  finish(error?: unknown): void {
    if (this.#finished) return; this.#finished = true;
    if (this.#requested && !this.#cancel && error === undefined) error = new TypeError("storage operation has no durable provider cancellation path");
    this.#error = error;
    if (error === undefined) this.#resolve(); else this.#reject(error);
  }
  #send(): Promise<void> { this.#cancelSent ??= this.#cancel!(); return this.#cancelSent; }
}

async function* failClosedResumeV2(resume: StorageV2Resume): AsyncIterable<never> {
  if (resume.kind === "provider-token") decodeHostV2("ResumeTokenV1", resume.token);
  throw new TypeError("semantic resume requires an exact successor outbox transition; intent replay is forbidden");
}

export class PrivateDurableBrowserIdentityV2Bridge implements IdentityV2Bridge {
  readonly #host: PrivateOriginBrowserRouterV2;
  constructor(host: PrivateOriginBrowserRouterV2) { this.#host = host; }
  async request<Op extends IdentityV2Call>(invocation: IdentityInvocationV2<Op>, signal?: AbortSignal): Promise<IdentityV2BridgeResult> {
    const contract = HOST_V2_OPERATION_BINDINGS[invocation.operation];
    const exact = encodeHostV2(contract.frame as HostV2TypeName, invocation.wireFrame);
    const result = await this.#host.invoke(invocation.operation, exact, undefined, signal);
    return result.error ? { success: false, error: result.error } : { success: true, value: result.value };
  }
}

/** Provider-side authenticated MessagePort pump. The Rust implementation remains the sole state authority. */
export async function runPrivateBrowserRustProviderV2(
  transport: BrowserHostV2Transport, bridge: PrivateBrowserRustProviderBridgeV2,
  options: BrowserHostV2IoOptions = {},
): Promise<void> {
  while (true) {
    const [request, authority] = await Promise.all([
      transport.receive("RequestV2", options), transport.receive("ProviderCapabilityV1", options),
    ]);
    const frame = decodeHostV2("RequestV2", request).value as WireMap;
    const operation = operationForCode(Number(frame[3]));
    if (!PROVIDER_BYTE_OPERATIONS.has(operation)) throw new TypeError("non-provider Host-v2 request reached the provider MessagePort");
    const length = Number(frame[3]) === 1010 ? BigInt((frame[8] as WireMap)[2] as bigint | number) : undefined;
    const upload = length === undefined ? undefined : receiveUpload(transport, bytes(frame[5], 16, "operation ID"), length, options);
    const cancellations = receiveProviderCancellations(transport, bytes(frame[1], 16, "request ID"), options);
    for await (const response of bridge.dispatch({ request, authority, ...(upload ? { upload } : {}), cancellations, ...(options.signal ? { signal: options.signal } : {}) })) {
      await transport.send("EventV2", response.event, options);
      const event = decodeHostV2("EventV2", response.event).value as WireMap;
      if ([2, 3, 4].includes(Number(event[3]))) {
        const ack = await transport.receive("ResponseAckV1", options);
        await bridge.acknowledge(ack, options.signal);
        break;
      }
    }
  }
}

async function* receiveProviderCancellations(
  transport: BrowserHostV2Transport, requestId: Uint8Array, options: BrowserHostV2IoOptions,
): AsyncIterable<{ readonly event: Uint8Array; readonly authority: Uint8Array }> {
  while (true) {
    const [event, authority] = await Promise.all([
      transport.receive("CancelledEventV2", options), transport.receive("ProviderCapabilityV1", options),
    ]);
    const decoded = decodeHostV2("CancelledEventV2", event).value;
    if (!equal(decoded[1], requestId) || Number(decoded[3]) !== 4) throw new TypeError("provider cancellation is not bound to the active request");
    decodeHostV2("ProviderCapabilityV1", authority);
    yield { event: event.slice(), authority: authority.slice() };
  }
}

async function* receiveUpload(transport: BrowserHostV2Transport, operationId: Uint8Array, declared: bigint, options: BrowserHostV2IoOptions): AsyncIterable<Uint8Array> {
  let received = 0n; let sequence = 0;
  while (received < declared) {
    const exact = await transport.receive("ProviderTransferChunkV1", options);
    const chunk = decodeHostV2("ProviderTransferChunkV1", exact).value;
    if (!equal(chunk[1], operationId) || Number(chunk[2]) !== sequence++) throw new TypeError("provider upload chunk binding is invalid");
    const digest = blake2b256(chunk[3]);
    if (!equal(digest, chunk[4])) throw new TypeError("provider upload chunk digest is invalid");
    received += BigInt(chunk[3].length);
    if (received > declared) throw new TypeError("provider upload exceeds declared length");
    yield exact;
  }
}

async function invokeExactHost<Op extends Operation>(
  bridge: PrivateExactHostBridgeV2<Op>, operation: Op, exactRequest: Uint8Array,
  signal?: AbortSignal, onEvent?: (event: Uint8Array) => void,
): Promise<PrivateInvocationResultV2> {
  const binding = HOST_V2_OPERATION_BINDINGS[operation];
  const request = decodeHostV2(binding.frame as HostV2TypeName, exactRequest).value as WireMap;
  if (Number(request[3]) !== binding.code) throw new TypeError("host/runtime request code mismatches its generated binding");
  const requestId = bytes(request[1], 16, "request ID");
  const authority = await bridge.finalizedAuthority({
    operation, code: binding.code, productId: request[2] as string, requestId,
  }, signal);
  bytes(authority.hash, 32, "finalized authority hash");
  if (!(authority.proof instanceof Uint8Array) || authority.proof.length === 0 || authority.number < 0n) {
    throw new TypeError("finalized host authority is invalid");
  }
  let sequence = 0; let accepted = false;
  for await (const output of bridge.dispatch({
    operation, request: exactRequest.slice(), authority,
    ...(signal ? { signal } : {}),
  })) {
    const event = decodeHostV2("EventV2", output.event).value as WireMap;
    if (!equal(bytes(event[1], 16, "event request ID"), requestId) || Number(event[2]) !== sequence++) {
      throw new TypeError("host/runtime event is not bound to the exact request sequence");
    }
    const kind = Number(event[3]); onEvent?.(output.event.slice());
    if (kind === 0) {
      if (accepted || sequence !== 1) throw new TypeError("host/runtime accepted event is duplicated or reordered");
      accepted = true; continue;
    }
    if (!accepted) throw new TypeError("host/runtime terminal or progress event preceded acceptance");
    if (kind === 1) continue;
    if (kind === 2) {
      encodeHostV2(binding.result as HostV2TypeName, event[4] as never);
      return { value: decodeResult(operation, event[4] as WireMap) };
    }
    if (kind === 3) {
      encodeHostV2(binding.error as HostV2TypeName, event[4] as never);
      if (!binding.allowedErrors.includes(Number((event[4] as WireMap)[0]) as never)) {
        throw new TypeError("host/runtime error is not allowed for the requested operation");
      }
      return { error: decodeError(event[4] as WireMap) };
    }
    throw new TypeError("host/runtime returned cancellation without a durable provider cancellation");
  }
  throw new TypeError("host/runtime execution ended without a terminal event");
}

function operationForCode(code: number): Operation {
  const match = (Object.entries(HOST_V2_OPERATION_BINDINGS) as readonly [Operation, (typeof HOST_V2_OPERATION_BINDINGS)[Operation]][])
    .find(([, binding]) => binding.code === code);
  if (!match) throw new TypeError("Host-v2 operation code is absent from the generated registry");
  return match[0];
}

function decodeStorageEvent(operation: StorageV2Operation, exact: Uint8Array): StorageV2Event {
  const event = decodeHostV2("EventV2", exact).value as WireMap;
  const requestId = event[1] as Uint8Array; const seq = Number(event[2]); const kind = Number(event[3]); const payload = event[4] as WireMap;
  if (kind === 0) return { kind: "accepted", requestId: requestId as never, seq: 0, state: Number(payload[0]) as 0 };
  if (kind === 1) return payload[1] instanceof Uint8Array
    ? { kind: "progress", requestId: requestId as never, seq, offset: BigInt(payload[0] as number | bigint), bytes: payload[1] } as StorageV2Event
    : { kind: "progress", requestId: requestId as never, seq, completed: BigInt(payload[0] as number | bigint), ...(payload[1] === undefined ? {} : { total: BigInt(payload[1] as number | bigint) }) } as StorageV2Event;
  if (kind === 2) return { kind: "result", requestId: requestId as never, seq, value: decodeResult(operation, payload) } as StorageV2Event;
  if (kind === 3) return { kind: "error", requestId: requestId as never, seq, ...decodeError(payload) };
  return { kind: "cancelled", requestId: requestId as never, seq };
}

function encodePrivateIntent(intent: PrivateStorageIntentV2): Uint8Array {
  return encodeHostV2(HOST_V2_OPERATION_BINDINGS[intent.operation].frame as HostV2TypeName, {
    0: 2, 1: intent.requestId, 2: intent.productId, 3: intent.code,
    ...(intent.grantId ? { 4: intent.grantId } : {}), ...(intent.operationId ? { 5: intent.operationId } : {}),
    7: intent.deadlineBlock, 8: privatePayload(intent.operation, intent.payload),
  });
}

function privatePayload(operation: StorageOperationV2, payload: Readonly<Record<string, unknown>>): HostV2Map {
  switch (operation) {
    case "storage.object.put": return { 0: payload.bucketId, 1: payload.cid, 2: payload.length, 3: payload.encrypted, 4: payload.transferId } as HostV2Map;
    case "storage.object.get": return { 0: payload.bucketId, 1: payload.cid } as HostV2Map;
    case "storage.object.status": return { 0: payload.bucketId, 1: payload.cid } as HostV2Map;
    case "storage.drive.commit": return { 0: payload.bucketId, 1: payload.manifest, 2: payload.bytes, 3: payload.expectedVersion, 4: payload.mode } as HostV2Map;
    case "storage.publish": return { 0: payload.nameHash, 1: payload.cid, ...(payload.expectedVersion === undefined ? {} : { 2: payload.expectedVersion }) } as unknown as HostV2Map;
    case "storage.resolve": return { 0: payload.name, ...(payload.version === undefined ? {} : { 1: payload.version }), ...(payload.at === undefined ? {} : { 2: payload.at }) } as unknown as HostV2Map;
  }
}

function finality(value: WireMap): unknown { return { number: BigInt(value[0] as number | bigint), hash: value[1] }; }
function identityFinality(value: WireMap): unknown { return { blockNumber: BigInt(value[0] as number | bigint), blockHash: value[1] }; }
function checkpoint(value: WireMap): unknown { return { root: value[0], from: BigInt(value[1] as number | bigint), to: BigInt(value[2] as number | bigint), replicas: Number(value[3]) }; }
function receipt(value: WireMap): unknown { return { provider: value[0], cid: value[1], length: BigInt(value[2] as number | bigint), signature: value[3] }; }
function identityReceipt(value: WireMap): unknown { return { commitment: value[0], validUntil: BigInt(value[1] as number | bigint), ...(value[2] ? { finalized: identityFinality(value[2] as WireMap) } : {}) }; }
function u64(value: unknown): bigint { return BigInt(value as number | bigint); }

function decodeResult(operation: Operation, p: WireMap): unknown {
  const fin = (key: number) => finality(p[key] as WireMap); const cp = (key: number) => checkpoint(p[key] as WireMap);
  switch (operation) {
    case "storage.bucket.create": return { bucketId: p[0], version: u64(p[1]), finalized: fin(2) };
    case "storage.bucket.get": { const b = p[0] as WireMap; return { owner: b[0], version: u64(b[1]), replicaCount: Number(b[2]), primary: b[3], providers: b[4], finalized: fin(1) }; }
    case "storage.bucket.grant": case "storage.bucket.revoke": case "storage.drive.share": return { grantId: p[0], version: u64(p[1]), finalized: fin(2) };
    case "storage.object.put": return { receipt: receipt(p[0] as WireMap), publishable: p[1], finalized: fin(2) };
    case "storage.object.get": return { cid: p[0], length: u64(p[1]), checkpoint: cp(2) };
    case "storage.object.range": return { cid: p[0], offset: u64(p[1]), length: u64(p[2]), total: u64(p[3]), checkpoint: cp(4) };
    case "storage.object.delete": return { version: u64(p[0]), pending: Number(p[1]), confirmed: Number(p[2]), finalized: fin(3) };
    case "storage.object.status": return { state: Number(p[0]), ...(p[1] ? { receipt: receipt(p[1] as WireMap) } : {}), ...(p[2] ? { checkpoint: cp(2) } : {}), replicas: Number(p[3]), publishable: p[4], finalized: fin(5) };
    case "storage.checkpoint.status": return { checkpoint: cp(0), sequence: Number(p[1]), block: u64(p[2]), quorum: Number(p[3]), finalized: fin(4) };
    case "storage.checkpoint.subscribe": case "storage.replica.subscribe": case "storage.deletion.subscribe": { const a = p[0] as WireMap; return { operationId: a[0], cursor: u64(a[1]) }; }
    case "storage.replica.status": return { primary: p[0], providers: p[1], healthy: Number(p[2]), lastCheckpoint: u64(p[3]), pending: Number(p[4]), finalized: fin(5) };
    case "storage.deletion.status": return { version: u64(p[0]), confirmations: Number(p[1]), root: p[2], finalized: fin(3) };
    case "storage.drive.read": return { manifest: p[0], entry: p[1], version: u64(p[2]), finalized: fin(3) };
    case "storage.drive.commit": return { manifest: p[0], version: u64(p[1]), checkpoint: cp(2), finalized: fin(3) };
    case "storage.s3.put": return { etag: p[0], version: u64(p[1]), finalized: fin(2) };
    case "storage.s3.get": return { cid: p[0], etag: p[1], version: u64(p[2]), finalized: fin(3) };
    case "storage.s3.list": return { cids: p[0], ...(p[1] ? { cursor: p[1] } : {}), version: u64(p[2]), finalized: fin(3) };
    case "storage.s3.delete": return { version: u64(p[0]), remainingHistory: Number(p[1]), finalized: fin(2) };
    case "storage.publish": return { nameHash: p[0], cid: p[1], finalized: fin(2) };
    case "storage.resolve": return { cid: p[0], version: u64(p[1]), checkpoint: cp(2), finalized: fin(3) };
    case "storage.keys.export": return { wrappedKey: p[0], algorithm: p[1], keyVersion: p[2] };
    case "storage.keys.import": return { keyId: p[0], keyVersion: p[1] };
    case "identity.account": return { account: p[0], sessionExpiresAt: u64(p[1]), finalized: identityFinality(p[2] as WireMap) };
    case "identity.profile.read": case "identity.profile.disclose": return { receipt: identityReceipt(p[0] as WireMap) };
    case "identity.humanity.status": return { status: Number(p[0]), freshUntil: u64(p[1]), finalized: identityFinality(p[2] as WireMap) };
    case "identity.humanity.prove": return { proof: p[0], derivedPublicKey: p[1], proofHash: p[2], continuity: p[3], expiresAt: u64(p[4]) };
    case "identity.subject.derive": return { subject: p[0], derivedPublicKey: p[1], epoch: Number(p[2]), recoveryIncarnationHash: p[3], continuity: p[4] };
    case "identity.entitlements.read": return { allowed: p[0], scope: p[1], policyVersion: Number(p[2]), expiresAt: u64(p[3]), freshUntil: u64(p[4]), finalized: identityFinality(p[5] as WireMap) };
    case "transaction.sign": return { transactionHash: p[0], finalized: identityFinality(p[1] as WireMap) };
  }
}

function decodeError(error: WireMap): PrivateBrowserErrorV2 {
  const code = Number(error[0]); const binding = HOST_V2_ERROR_BINDINGS[String(code) as keyof typeof HOST_V2_ERROR_BINDINGS];
  if (!binding) throw new TypeError("Host-v2 error is not in the generated registry");
  if (error[1] !== binding.name || error[2] !== binding.retryable) throw new TypeError("Host-v2 error metadata mismatches the generated registry");
  const wire = error[3] as WireMap | undefined;
  const details = wire === undefined ? undefined : {
    ...(typeof wire[0] === "string" ? { message: wire[0] } : {}),
    ...(wire[1] === undefined ? {} : { lower: BigInt(wire[1] as number | bigint) }),
    ...(wire[2] === undefined ? {} : { upper: BigInt(wire[2] as number | bigint) }),
    ...(wire[3] instanceof Uint8Array ? { hash: wire[3].slice() } : {}),
  };
  return { code, name: binding.name, retryable: binding.retryable, ...(details ? { details } : {}) };
}
function io(signal?: AbortSignal): BrowserHostV2IoOptions { return signal ? { signal } : {}; }
function bytes(value: unknown, length: number, label: string): Uint8Array { if (!(value instanceof Uint8Array) || value.length !== length) throw new TypeError(`${label} must contain ${length} bytes`); return value.slice(); }
function concat(left: Uint8Array, right: Uint8Array): Uint8Array { const joined = new Uint8Array(left.length + right.length); joined.set(left); joined.set(right, left.length); return joined; }
function equal(left: Uint8Array, right: Uint8Array): boolean { return left.length === right.length && left.every((value, index) => value === right[index]); }
