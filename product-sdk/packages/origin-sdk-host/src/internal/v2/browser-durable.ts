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

import { decodeHostV2 } from "./codec.ts";
import {
  BrowserHostOutboxV1,
  type BrowserOutboxRetryV1,
  type BrowserPrepareOutboxV1,
} from "./browser-outbox.ts";
import { BrowserHostV2Transport } from "./browser.ts";
import { HostV2Session } from "./session.ts";

export type DurableBrowserEvent =
  | { readonly terminal: false; readonly event: Uint8Array }
  | { readonly terminal: true; readonly event: Uint8Array; readonly responseHash: Uint8Array };

export class DurableBrowserHostV2 {
  readonly #transport: BrowserHostV2Transport;
  readonly #outbox: BrowserHostOutboxV1;
  #active?: { readonly session: HostV2Session; readonly outboxId: Uint8Array };

  constructor(transport: BrowserHostV2Transport, outbox: BrowserHostOutboxV1) {
    this.#transport = transport;
    this.#outbox = outbox;
  }

  async prepareAndSend(input: BrowserPrepareOutboxV1): Promise<BrowserOutboxRetryV1> {
    const retry = await this.#outbox.prepare(input);
    this.#begin(retry.requestId, retry.outboxId);
    try {
      await this.#sendRetry(retry);
      await this.#outbox.markSent(retry.outboxId);
    } catch (error) {
      this.#transport.close();
      throw error;
    }
    return retry;
  }

  async resumeAndSend(outboxId: Uint8Array, finalized: bigint): Promise<BrowserOutboxRetryV1> {
    const retry = this.#outbox.retry(outboxId, finalized);
    this.#begin(retry.requestId, retry.outboxId);
    try {
      await this.#sendRetry(retry);
    } catch (error) {
      this.#transport.close();
      throw error;
    }
    return retry;
  }

  async receiveEvent(terminalBlock: bigint): Promise<DurableBrowserEvent> {
    const bytes = await this.#transport.receive("EventV2");
    const active = this.#active;
    if (!active) throw new Error("browser host-v2 session has not sent a durable request");
    active.session.accept(bytes);
    if (!active.session.isTerminal) return { terminal: false, event: bytes.slice() };
    let installed: { readonly responseHash: Uint8Array; readonly ack: Uint8Array };
    try {
      installed = await this.#outbox.installTerminal(active.outboxId, bytes, terminalBlock);
      await this.#transport.send("ResponseAckV1", installed.ack);
    } catch (error) {
      this.#transport.close();
      throw error;
    }
    return { terminal: true, event: bytes.slice(), responseHash: installed.responseHash };
  }

  async resumeAck(outboxId: Uint8Array): Promise<Uint8Array> {
    const installed = this.#outbox.installedAck(outboxId);
    try {
      await this.#transport.send("ResponseAckV1", installed.ack);
    } catch (error) {
      this.#transport.close();
      throw error;
    }
    return installed.responseHash;
  }

  async confirmAndGc(
    outboxId: Uint8Array,
    responseHash: Uint8Array,
    finalized: bigint,
  ): Promise<number> {
    await this.#outbox.confirmAck(outboxId, responseHash);
    return this.#outbox.gc(finalized, 1);
  }

  async #sendRetry(retry: BrowserOutboxRetryV1): Promise<void> {
    const authority = authorityProduction(retry.authority);
    await Promise.all([
      this.#transport.send("RequestV2", retry.request),
      this.#transport.send(authority, retry.authority),
    ]);
  }

  #begin(requestId: Uint8Array, outboxId: Uint8Array): void {
    if (this.#active) throw new Error("browser host-v2 transport already has a session");
    this.#active = {
      session: new HostV2Session(this.#transport.negotiation, requestId),
      outboxId: outboxId.slice(),
    };
  }
}

function authorityProduction(bytes: Uint8Array): "ProviderCapabilityV1" | "ResumeTokenV1" {
  try {
    decodeHostV2("ProviderCapabilityV1", bytes);
    return "ProviderCapabilityV1";
  } catch {
    decodeHostV2("ResumeTokenV1", bytes);
    return "ResumeTokenV1";
  }
}
