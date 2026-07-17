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

import { decodeHostV2, HostV2CodecError } from "./codec.ts";
import type { EventV2, RequestId } from "./generated.ts";

export class HostV2SessionError extends Error {
  readonly code = "WIRE_SEQUENCE_INVALID" as const;

  constructor(message: string) {
    super(message);
    this.name = "HostV2SessionError";
  }
}

function equalBytes(left: Uint8Array, right: Uint8Array): boolean {
  return left.length === right.length && left.every((value, index) => value === right[index]);
}

export class HostV2Session {
  private nextSequence = 0;
  private terminal = false;
  private accepted = false;
  private readonly requestId: RequestId;

  constructor(requestId: RequestId) {
    if (!(requestId instanceof Uint8Array) || requestId.length !== 16) {
      throw new HostV2CodecError("WIRE_SCHEMA_INVALID", "host-v2 request ID must be exactly 16 bytes");
    }
    this.requestId = requestId;
  }

  get isTerminal(): boolean {
    return this.terminal;
  }

  accept(bytes: Uint8Array): EventV2 {
    if (this.terminal) throw new HostV2SessionError("event received after terminal host-v2 event");
    const event = decodeHostV2("EventV2", bytes).value;
    if (!equalBytes(event[1], this.requestId)) throw new HostV2SessionError("event request ID does not match session");
    const sequence = Number(event[2]);
    if (!Number.isSafeInteger(sequence) || sequence !== this.nextSequence) {
      throw new HostV2SessionError("host-v2 event sequence is duplicated or skipped");
    }
    const kind = event[3];
    if (sequence === 0 && kind !== 0) throw new HostV2SessionError("first host-v2 event must be accepted");
    if (sequence !== 0 && kind === 0) throw new HostV2SessionError("accepted host-v2 event may occur only once");
    if (kind === 0) this.accepted = true;
    if (!this.accepted) throw new HostV2SessionError("host-v2 event preceded acceptance");
    this.nextSequence += 1;
    this.terminal = kind === 2 || kind === 3 || kind === 4;
    return event;
  }
}
