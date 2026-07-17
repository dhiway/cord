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
  | { readonly terminal: false; readonly event: Uint8Array }
  | { readonly terminal: true; readonly event: Uint8Array; readonly responseHash: Uint8Array };
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
  constructor(transport: BrowserHostV2Transport, outbox: BrowserHostOutboxV1) {
    const transportBinding = transport.binding; const outboxBinding = outbox.contextBinding;
    if (!equal(transportBinding.registryHash, outboxBinding.registryHash) || !equal(transportBinding.genesisHash, outboxBinding.genesisHash)
      || !equal(transportBinding.negotiatedTuple, outboxBinding.negotiatedTuple) || !equal(transportBinding.providerId, outboxBinding.providerId)
      || !equal(transportBinding.providerEndpointHash, outboxBinding.providerEndpointHash)) throw new Error("browser durable host-v2 transport and outbox bindings differ");
    this.#transport = transport; this.#outbox = outbox;
  }
  async prepareAndSend(input: BrowserPrepareOutboxV1, options: BrowserHostV2IoOptions = {}): Promise<BrowserOutboxRetryV1> {
    const retry = await this.#outbox.prepare(input); this.#begin(retry);
    try { await this.#sendRetry(retry, options); await this.#outbox.markSent(retry.outboxId); } catch (error) { this.#transport.close(); throw error; }
    return retry;
  }
  async resumeAndSend(outboxId: Uint8Array, finalized: bigint, options: BrowserHostV2IoOptions = {}): Promise<BrowserOutboxRetryV1> {
    const retry = this.#outbox.retry(outboxId, finalized); this.#begin(retry);
    try { await this.#sendRetry(retry, options); } catch (error) { this.#transport.close(); throw error; } return retry;
  }
  async prepareCancelAndSend(exactCancel: Uint8Array, options: BrowserHostV2IoOptions = {}): Promise<BrowserOutboxRetryV1> {
    const active = this.#active; if (!active || active.session.isClosed) throw new Error("browser host-v2 session cannot prepare cancel");
    const retry = await this.#outbox.prepareCancel(active.outboxId, exactCancel, active.session.nextExpectedSequence);
    try { await this.#sendRetry(retry, options); await this.#outbox.markSent(retry.outboxId); } catch (error) { this.#transport.close(); throw error; }
    this.#active = { ...active, expectedResponseKind: 4 }; return retry;
  }
  async receiveEvent(terminalBlock: bigint, options: BrowserHostV2IoOptions = {}): Promise<DurableBrowserEvent> {
    const bytes = await this.#transport.receive("EventV2", options); const active = this.#active;
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
      this.#active = undefined;
      return { terminal: true, event: bytes.slice(), responseHash: installed.responseHash };
    } catch (error) { this.#transport.close(); throw error; }
  }
  async sendProviderTransferChunk(exactChunk: Uint8Array, options: BrowserHostV2IoOptions = {}): Promise<void> {
    await this.#transport.send("ProviderTransferChunkV1", exactChunk, options);
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
    return this.#outbox.gc(finalized, 1);
  }
  async #sendRetry(retry: BrowserOutboxRetryV1, options: BrowserHostV2IoOptions): Promise<void> {
    const authority = authorityProduction(retry.authority); const request = retry.cancel ? "CancelledEventV2" : "RequestV2";
    await Promise.all([this.#transport.send(request, retry.request, options), this.#transport.send(authority, retry.authority, options)]);
  }
  #begin(retry: BrowserOutboxRetryV1): void {
    if (this.#active) throw new Error("browser host-v2 transport already has a session");
    this.#active = {
      session: retry.cancel ? HostV2Session.resume(this.#transport.negotiation, retry.requestId, retry.intendedCursor) : new HostV2Session(this.#transport.negotiation, retry.requestId),
      outboxId: retry.outboxId.slice(), operationId: retry.operationId.slice(), expectedResponseKind: retry.expectedResponseKind, operationCode: retry.operationCode,
    };
  }
}
function authorityProduction(bytes: Uint8Array): "ProviderCapabilityV1" | "ResumeTokenV1" { try { decodeHostV2("ProviderCapabilityV1", bytes); return "ProviderCapabilityV1"; } catch { decodeHostV2("ResumeTokenV1", bytes); return "ResumeTokenV1"; } }
function equal(left: Uint8Array, right: Uint8Array): boolean { return left.length === right.length && left.every((byte, index) => byte === right[index]); }
