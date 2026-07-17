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
import {
  BrowserHostOutboxV1,
  providerAckConfirmationMessage,
  type BrowserOutboxRetryV1,
  type BrowserPrepareOutboxV1,
} from "./browser-outbox.ts";
import { BrowserHostV2Transport, type BrowserHostV2IoOptions } from "./browser.ts";
import { HostV2Session } from "./session.ts";
import { HOST_V2_OPERATION_BINDINGS, type HostV2TypeName } from "./generated.ts";

export type DurableBrowserEvent =
  | { readonly terminal: false; readonly event: Uint8Array; readonly successorToken?: Uint8Array }
  | { readonly terminal: true; readonly event: Uint8Array; readonly responseHash: Uint8Array; readonly outboxId: Uint8Array };
export interface BrowserProviderAckConfirmationV1 {
  readonly outboxId: Uint8Array; readonly responseHash: Uint8Array; readonly signature: Uint8Array;
}
interface ActiveBrowserRequest {
  readonly session: HostV2Session; readonly outboxId: Uint8Array; readonly operationId: Uint8Array;
  readonly expectedResponseKind: number;
  readonly operationCode: number;
}

export class DurableBrowserHostV2 {
  readonly #transport: BrowserHostV2Transport; readonly #outbox: BrowserHostOutboxV1;
  #active: ActiveBrowserRequest | undefined;
  #pendingSuccessorEvent: Uint8Array | undefined;
  constructor(transport: BrowserHostV2Transport, outbox: BrowserHostOutboxV1) {
    const transportBinding = transport.binding; const outboxBinding = outbox.contextBinding;
    if (!equal(transportBinding.registryHash, outboxBinding.registryHash) || !equal(transportBinding.genesisHash, outboxBinding.genesisHash)
      || !equal(transportBinding.negotiatedTuple, outboxBinding.negotiatedTuple) || !equal(transportBinding.providerId, outboxBinding.providerId)
      || !equal(transportBinding.providerEndpointHash, outboxBinding.providerEndpointHash)) throw new Error("browser durable host-v2 transport and outbox bindings differ");
    this.#transport = transport; this.#outbox = outbox;
  }
  async prepareAndSend(input: BrowserPrepareOutboxV1, options: BrowserHostV2IoOptions = {}): Promise<BrowserOutboxRetryV1> {
    const retry = await this.#outbox.prepare(input); this.#begin(retry);
    try { await this.#sendRetry(retry, options); await this.#outbox.markSent(retry.outboxId); } catch (error) { this.#resetAfterTransportFailure(); throw error; }
    return retry;
  }
  async prepareSuccessorAndSend(predecessorOutboxId: Uint8Array, input: BrowserPrepareOutboxV1, options: BrowserHostV2IoOptions = {}): Promise<BrowserOutboxRetryV1> {
    const retry = await this.#outbox.prepareSuccessor(predecessorOutboxId, input); this.#begin(retry);
    try { await this.#sendRetry(retry, options); await this.#outbox.markSent(retry.outboxId); } catch (error) { this.#resetAfterTransportFailure(); throw error; }
    return retry;
  }
  async resumeAndSend(outboxId: Uint8Array, finalized: bigint, options: BrowserHostV2IoOptions = {}): Promise<BrowserOutboxRetryV1> {
    const retry = this.#outbox.retry(outboxId, finalized); this.#begin(retry);
    try { await this.#sendRetry(retry, options); } catch (error) { this.#resetAfterTransportFailure(); throw error; } return retry;
  }
  async resumeLinkedSuccessorAndSend(predecessorOutboxId: Uint8Array, finalized: bigint, options: BrowserHostV2IoOptions = {}): Promise<BrowserOutboxRetryV1 | undefined> {
    const retry = this.#outbox.linkedSuccessor(predecessorOutboxId, finalized);
    if (!retry) return undefined;
    this.#begin(retry);
    try { await this.#sendRetry(retry, options); }
    catch (error) { this.#resetAfterTransportFailure(); throw error; }
    return retry;
  }
  async prepareCancelAndSend(exactCancel: Uint8Array, options: BrowserHostV2IoOptions = {}): Promise<BrowserOutboxRetryV1> {
    const active = this.#active; if (!active || active.session.isClosed) throw new Error("browser host-v2 session cannot prepare cancel");
    const retry = await this.#outbox.prepareCancel(active.outboxId, exactCancel, active.session.nextExpectedSequence);
    try { await this.#sendRetry(retry, options); await this.#outbox.markSent(retry.outboxId); } catch (error) { this.#resetAfterTransportFailure(); throw error; }
    this.#active = { ...active, expectedResponseKind: 4 }; return retry;
  }
  async receiveEvent(terminalBlock: bigint, options: BrowserHostV2IoOptions = {}): Promise<DurableBrowserEvent> {
    if (this.#pendingSuccessorEvent) throw new Error("browser successor authority is not durably installed");
    return this.#acceptEvent(await this.#transport.receive("EventV2", options), terminalBlock, options);
  }
  async receiveProviderEvent(terminalBlock: bigint, options: BrowserHostV2IoOptions = {}): Promise<DurableBrowserEvent> {
    if (this.#pendingSuccessorEvent) throw new Error("browser successor authority is not durably installed");
    const first = await this.#transport.receiveOneOf(["EventV2", "ResumeTokenV1"], options);
    if (first.production !== "EventV2") { this.#transport.close(); throw new Error("browser provider sent ResumeTokenV1 before its EventV2"); }
    const event = await this.#acceptEvent(first.bytes, terminalBlock, options); if (event.terminal) return event;
    const second = await this.#transport.receiveOneOf(["EventV2", "ResumeTokenV1"], options);
    if (second.production !== "ResumeTokenV1") { this.#transport.close(); throw new Error("browser provider omitted the exact successor after nonterminal EventV2"); }
    this.#pendingSuccessorEvent = event.event.slice(); return { ...event, successorToken: second.bytes.slice() };
  }
  async installSuccessor(exactResumeToken: Uint8Array, cursor: number, options: BrowserHostV2IoOptions = {}): Promise<{ readonly outboxId: Uint8Array; readonly responseHash: Uint8Array }> {
    const active = this.#active; const event = this.#pendingSuccessorEvent;
    if (!active || !event) throw new Error("browser successor response is not pending durable installation");
    try {
      const installed = await this.#outbox.installSuccessor(active.outboxId, event, exactResumeToken, cursor);
      await this.#transport.send("ResponseAckV1", installed.ack, options);
      const outboxId = active.outboxId.slice(); this.#active = undefined; this.#pendingSuccessorEvent = undefined;
      return { outboxId, responseHash: installed.responseHash };
    } catch (error) { this.#transport.close(); throw error; }
  }
  async #acceptEvent(bytes: Uint8Array, terminalBlock: bigint, options: BrowserHostV2IoOptions): Promise<DurableBrowserEvent> {
    const active = this.#active;
    if (!active) throw new Error("browser host-v2 session has not sent a durable request");
    const event = active.session.accept(bytes);
    if (!active.session.isTerminal) return { terminal: false, event: bytes.slice() };
    if (event[3] !== 3 && event[3] !== active.expectedResponseKind) { this.#transport.close(); throw new Error("browser terminal result kind mismatches durable request"); }
    const operation = Object.values(HOST_V2_OPERATION_BINDINGS).find(({ code }) => code === active.operationCode);
    if (!operation) { this.#transport.close(); throw new Error("browser durable operation binding is unknown"); }
    try {
      if (event[3] === 2) encodeHostV2(operation.result as HostV2TypeName, event[4]);
      if (event[3] === 3) {
        encodeHostV2(operation.error as HostV2TypeName, event[4]);
        const code = Number((event[4] as Record<number, unknown>)[0]);
        if (!operation.allowedErrors.includes(code as never)) throw new Error();
      }
    } catch { this.#transport.close(); throw new Error("browser terminal payload mismatches the requested operation"); }
    try {
      const installed = await this.#outbox.installTerminal(active.outboxId, bytes, terminalBlock);
      await this.#transport.send("ResponseAckV1", installed.ack, options);
      const outboxId = active.outboxId.slice();
      this.#active = undefined;
      return { terminal: true, event: bytes.slice(), responseHash: installed.responseHash, outboxId };
    } catch (error) { this.#transport.close(); throw error; }
  }
  async resumeAck(outboxId: Uint8Array, options: BrowserHostV2IoOptions = {}): Promise<Uint8Array> {
    const installed = this.#outbox.installedAck(outboxId);
    try { await this.#transport.send("ResponseAckV1", installed.ack, options); } catch (error) { this.#transport.close(); throw error; }
    return installed.responseHash;
  }
  async confirmAndGc(confirmation: BrowserProviderAckConfirmationV1, finalized: bigint): Promise<number> {
    const binding = this.#transport.binding; const context = this.#outbox.contextBinding;
    const message = providerAckConfirmationMessage(context, confirmation.outboxId, confirmation.responseHash);
    if (!await this.#outbox.verifyProviderAck(binding.acknowledgementPublicKey, message, confirmation.signature)) throw new Error("browser provider acknowledgement confirmation is unauthenticated");
    await this.#outbox.confirmAck(confirmation.outboxId, confirmation.responseHash);
    await this.#outbox.retireUploadSpool(confirmation.outboxId);
    return this.#outbox.gc(finalized, 1);
  }
  async #sendRetry(retry: BrowserOutboxRetryV1, options: BrowserHostV2IoOptions): Promise<void> {
    const authority = authorityProduction(retry.authority); const request = retry.cancel ? "CancelledEventV2" : "RequestV2";
    await Promise.all([this.#transport.send(request, retry.request, options), this.#transport.send(authority, retry.authority, options)]);
    if (retry.uploadChunk) await this.#transport.send("ProviderTransferChunkV1", retry.uploadChunk, options);
  }
  #begin(retry: BrowserOutboxRetryV1): void {
    if (this.#active) throw new Error("browser host-v2 transport already has a session");
    this.#active = {
      session: retry.cancel || authorityProduction(retry.authority) === "ResumeTokenV1"
        ? HostV2Session.resume(this.#transport.negotiation, retry.requestId, retry.intendedCursor)
        : new HostV2Session(this.#transport.negotiation, retry.requestId),
      outboxId: retry.outboxId.slice(), operationId: retry.operationId.slice(), expectedResponseKind: retry.expectedResponseKind, operationCode: retry.operationCode,
    };
  }
  #resetAfterTransportFailure(): void { this.#active = undefined; this.#pendingSuccessorEvent = undefined; this.#transport.close(); }
}
function authorityProduction(bytes: Uint8Array): "ProviderCapabilityV1" | "ResumeTokenV1" { try { decodeHostV2("ProviderCapabilityV1", bytes); return "ProviderCapabilityV1"; } catch { decodeHostV2("ResumeTokenV1", bytes); return "ResumeTokenV1"; } }
function equal(left: Uint8Array, right: Uint8Array): boolean { return left.length === right.length && left.every((byte, index) => byte === right[index]); }
