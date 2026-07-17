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
import {
  HOST_V2_FEATURE_IDS,
  HOST_V2_MAJOR,
  HOST_V2_MINOR,
  HOST_V2_PROTOCOL,
  HOST_V2_REGISTRY_SHA256,
  type EventV2,
  type HostV2FeatureId,
  type RequestId,
} from "./generated.ts";

export interface HostV2NegotiationOffer {
  readonly protocol: typeof HOST_V2_PROTOCOL;
  readonly major: number;
  readonly minors: readonly number[];
  readonly genesis: Uint8Array;
  readonly finalizedSpecVersion: number;
  readonly finalizedTransactionVersion: number;
  readonly registrySha256: string;
  readonly features: readonly HostV2FeatureId[];
}

const negotiatedAuthority = Symbol("cord.origin.host/2 negotiated authority");

class NegotiatedHostV2State {
  readonly #authority = "cord.origin.host/2:negotiated";
  readonly #minor: number;
  readonly #genesis: Uint8Array;
  readonly #finalizedSpecVersion: number;
  readonly #finalizedTransactionVersion: number;
  readonly #features: readonly HostV2FeatureId[];

  constructor(
    authority: symbol,
    minor: number,
    genesis: Uint8Array,
    finalizedSpecVersion: number,
    finalizedTransactionVersion: number,
    features: readonly HostV2FeatureId[],
  ) {
    if (authority !== negotiatedAuthority) {
      throw new HostV2NegotiationError(
        "WIRE_DESCRIPTOR_MISMATCH",
        "host-v2 negotiation authority is private",
      );
    }
    this.#minor = minor;
    this.#genesis = genesis.slice();
    this.#finalizedSpecVersion = finalizedSpecVersion;
    this.#finalizedTransactionVersion = finalizedTransactionVersion;
    this.#features = Object.freeze([...features]);
    Object.freeze(this);
  }

  static snapshot(value: HostV2Negotiated): NegotiatedHostV2State {
    try {
      if (value.#authority !== "cord.origin.host/2:negotiated") throw new Error("invalid authority");
      return createNegotiatedHostV2State(
        value.#minor,
        value.#genesis,
        value.#finalizedSpecVersion,
        value.#finalizedTransactionVersion,
        value.#features,
      );
    } catch {
      throw new HostV2NegotiationError(
        "WIRE_DESCRIPTOR_MISMATCH",
        "session requires an opaque validated host-v2 negotiation",
      );
    }
  }

  get protocol(): typeof HOST_V2_PROTOCOL {
    return HOST_V2_PROTOCOL;
  }

  get major(): typeof HOST_V2_MAJOR {
    return HOST_V2_MAJOR;
  }

  get minor(): number {
    return this.#minor;
  }

  get genesis(): Uint8Array {
    return this.#genesis.slice();
  }

  get finalizedSpecVersion(): number {
    return this.#finalizedSpecVersion;
  }

  get finalizedTransactionVersion(): number {
    return this.#finalizedTransactionVersion;
  }

  get registrySha256(): typeof HOST_V2_REGISTRY_SHA256 {
    return HOST_V2_REGISTRY_SHA256;
  }

  get features(): readonly HostV2FeatureId[] {
    return Object.freeze([...this.#features]);
  }
}

export type HostV2Negotiated = NegotiatedHostV2State;

function createNegotiatedHostV2State(
  minor: number,
  genesis: Uint8Array,
  finalizedSpecVersion: number,
  finalizedTransactionVersion: number,
  features: readonly HostV2FeatureId[],
): NegotiatedHostV2State {
  return new NegotiatedHostV2State(
    negotiatedAuthority,
    minor,
    genesis,
    finalizedSpecVersion,
    finalizedTransactionVersion,
    features,
  );
}

export class HostV2NegotiationError extends Error {
  readonly code: "WIRE_VERSION_MISMATCH" | "WIRE_GENESIS_MISMATCH" | "WIRE_DESCRIPTOR_MISMATCH";

  constructor(code: HostV2NegotiationError["code"], message: string) {
    super(message);
    this.name = "HostV2NegotiationError";
    this.code = code;
  }
}

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

function version(value: number, label: string, max = 0xffff_ffff): number {
  if (!Number.isSafeInteger(value) || value < 0 || value > max) {
    throw new HostV2NegotiationError("WIRE_VERSION_MISMATCH", `${label} is outside the supported unsigned range`);
  }
  return value;
}

function validateOffer(offer: HostV2NegotiationOffer, side: string): void {
  if (offer.protocol !== HOST_V2_PROTOCOL || offer.major !== HOST_V2_MAJOR) {
    throw new HostV2NegotiationError("WIRE_VERSION_MISMATCH", `${side} host-v2 protocol major is incompatible`);
  }
  if (offer.minors.length === 0) {
    throw new HostV2NegotiationError("WIRE_VERSION_MISMATCH", `${side} host-v2 minor set is empty`);
  }
  const minors = new Set<number>();
  for (const minor of offer.minors) {
    version(minor, `${side} minor`, HOST_V2_MINOR);
    if (minors.has(minor)) throw new HostV2NegotiationError("WIRE_VERSION_MISMATCH", `${side} minor set contains duplicates`);
    minors.add(minor);
  }
  if (!(offer.genesis instanceof Uint8Array) || offer.genesis.length !== 32) {
    throw new HostV2NegotiationError("WIRE_GENESIS_MISMATCH", `${side} genesis must contain exactly 32 bytes`);
  }
  version(offer.finalizedSpecVersion, `${side} finalized spec version`);
  version(offer.finalizedTransactionVersion, `${side} finalized transaction version`);
  if (offer.registrySha256 !== HOST_V2_REGISTRY_SHA256) {
    throw new HostV2NegotiationError("WIRE_DESCRIPTOR_MISMATCH", `${side} registry descriptor hash is incompatible`);
  }
  const knownFeatures = new Set<string>(HOST_V2_FEATURE_IDS);
  const features = new Set<string>();
  for (const feature of offer.features) {
    if (!knownFeatures.has(feature) || features.has(feature)) {
      throw new HostV2NegotiationError("WIRE_DESCRIPTOR_MISMATCH", `${side} feature set is unknown or duplicated`);
    }
    features.add(feature);
  }
}

export function negotiateHostV2(
  local: HostV2NegotiationOffer,
  remote: HostV2NegotiationOffer,
): HostV2Negotiated {
  validateOffer(local, "local");
  validateOffer(remote, "remote");
  if (!equalBytes(local.genesis, remote.genesis)) {
    throw new HostV2NegotiationError("WIRE_GENESIS_MISMATCH", "host-v2 peers advertise different chain genesis hashes");
  }
  if (local.finalizedSpecVersion !== remote.finalizedSpecVersion
    || local.finalizedTransactionVersion !== remote.finalizedTransactionVersion) {
    throw new HostV2NegotiationError("WIRE_VERSION_MISMATCH", "host-v2 peers advertise different finalized runtime versions");
  }
  const remoteMinors = new Set(remote.minors);
  const minor = [...local.minors].filter((candidate) => remoteMinors.has(candidate)).sort((a, b) => b - a)[0];
  if (minor === undefined) {
    throw new HostV2NegotiationError("WIRE_VERSION_MISMATCH", "host-v2 peers have no common minor version");
  }
  const remoteFeatures = new Set(remote.features);
  const features = HOST_V2_FEATURE_IDS.filter((feature) => local.features.includes(feature) && remoteFeatures.has(feature));
  return createNegotiatedHostV2State(
    minor,
    local.genesis,
    local.finalizedSpecVersion,
    local.finalizedTransactionVersion,
    features,
  );
}

export class HostV2Session {
  private nextSequence = 0;
  private terminal = false;
  private accepted = false;
  private closed = false;
  private readonly requestId: RequestId;
  private readonly negotiated: HostV2Negotiated;

  constructor(negotiated: HostV2Negotiated, requestId: RequestId) {
    if (!(requestId instanceof Uint8Array) || requestId.length !== 16) {
      throw new HostV2CodecError("WIRE_SCHEMA_INVALID", "host-v2 request ID must be exactly 16 bytes");
    }
    this.negotiated = NegotiatedHostV2State.snapshot(negotiated);
    this.requestId = requestId.slice() as RequestId;
  }

  get isTerminal(): boolean {
    return this.terminal;
  }

  get isClosed(): boolean {
    return this.closed;
  }

  get negotiation(): HostV2Negotiated {
    return NegotiatedHostV2State.snapshot(this.negotiated);
  }

  private sequenceFault(message: string): never {
    this.closed = true;
    throw new HostV2SessionError(message);
  }

  accept(bytes: Uint8Array): EventV2 {
    if (this.closed) throw new HostV2SessionError("host-v2 session is permanently closed");
    let event: EventV2;
    try {
      event = decodeHostV2("EventV2", bytes).value;
    } catch (error) {
      this.closed = true;
      throw error;
    }
    if (!equalBytes(event[1], this.requestId)) return this.sequenceFault("event request ID does not match session");
    const sequence = Number(event[2]);
    if (!Number.isSafeInteger(sequence) || sequence !== this.nextSequence) {
      return this.sequenceFault("host-v2 event sequence is duplicated or skipped");
    }
    const kind = event[3];
    if (sequence === 0 && kind !== 0) return this.sequenceFault("first host-v2 event must be accepted");
    if (sequence !== 0 && kind === 0) return this.sequenceFault("accepted host-v2 event may occur only once");
    if (kind === 0) this.accepted = true;
    if (!this.accepted) return this.sequenceFault("host-v2 event preceded acceptance");
    this.nextSequence += 1;
    this.terminal = kind === 2 || kind === 3 || kind === 4;
    this.closed = this.terminal;
    return event;
  }
}
