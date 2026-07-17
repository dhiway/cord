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

import { encodeHostV2Value } from "./codec.ts";

const ENVELOPE_VERSION = 1;
const TAG_BYTES = 16;
const MASK_128 = (1n << 128n) - 1n;
const POLY_MODULUS = (1n << 130n) - 5n;

export interface BrowserHostOutboxContextV1 {
  readonly profileId: Uint8Array;
  readonly registryHash: Uint8Array;
  readonly genesisHash: Uint8Array;
  readonly negotiatedTuple: Uint8Array;
  readonly providerId: Uint8Array;
  readonly providerEndpointHash: Uint8Array;
}

export type BrowserRandomBytes = (length: number) => Uint8Array;

export class BrowserHostOutboxKeyRingV1 {
  readonly #activeVersion: number;
  readonly #keys: ReadonlyMap<number, Uint8Array>;

  constructor(activeVersion: number, keys: ReadonlyMap<number, Uint8Array>) {
    if (!Number.isSafeInteger(activeVersion) || activeVersion < 0 || !keys.has(activeVersion)) {
      throw new TypeError("browser host outbox active key is unavailable");
    }
    const copied = new Map<number, Uint8Array>();
    for (const [version, key] of keys) {
      if (!Number.isSafeInteger(version) || version < 0 || !(key instanceof Uint8Array) || key.length !== 32) {
        throw new TypeError("browser host outbox key ring is invalid");
      }
      copied.set(version, key.slice());
    }
    this.#activeVersion = activeVersion;
    this.#keys = copied;
    Object.freeze(this);
  }

  get activeVersion(): number {
    return this.#activeVersion;
  }

  key(version: number): Uint8Array {
    const key = this.#keys.get(version);
    if (!key) throw new TypeError("browser host outbox key version is unavailable");
    return key.slice();
  }
}

export class BrowserXChaCha20Poly1305 {
  readonly #context: BrowserHostOutboxContextV1;
  readonly #keys: BrowserHostOutboxKeyRingV1;
  readonly #crypto: Crypto;
  readonly #random: BrowserRandomBytes;

  constructor(
    context: BrowserHostOutboxContextV1,
    keys: BrowserHostOutboxKeyRingV1,
    crypto: Crypto = globalThis.crypto,
    random: BrowserRandomBytes = (length) => crypto.getRandomValues(new Uint8Array(length)),
  ) {
    validateContext(context);
    if (!crypto?.subtle) throw new TypeError("browser Web Crypto is unavailable");
    this.#context = copyContext(context);
    this.#keys = keys;
    this.#crypto = crypto;
    this.#random = random;
  }

  get activeKeyVersion(): number {
    return this.#keys.activeVersion;
  }

  async seal(id: Uint8Array, plaintext: Uint8Array): Promise<{ readonly keyVersion: number; readonly ciphertext: Uint8Array }> {
    exactBytes(id, 16, "outbox ID");
    const keyVersion = this.#keys.activeVersion;
    const nonce = this.#random(24);
    exactBytes(nonce, 24, "XChaCha20 nonce");
    const aad = outboxAad(this.#context, id, keyVersion);
    const sealed = xchacha20poly1305Seal(this.#keys.key(keyVersion), nonce, aad, plaintext);
    const ciphertext = new Uint8Array(1 + nonce.length + sealed.length);
    ciphertext[0] = ENVELOPE_VERSION;
    ciphertext.set(nonce, 1);
    ciphertext.set(sealed, 25);
    return { keyVersion, ciphertext };
  }

  async open(id: Uint8Array, keyVersion: number, envelope: Uint8Array): Promise<Uint8Array> {
    exactBytes(id, 16, "outbox ID");
    if (envelope.length < 1 + 24 + TAG_BYTES || envelope[0] !== ENVELOPE_VERSION) {
      throw new TypeError("browser host outbox envelope is invalid");
    }
    const aad = outboxAad(this.#context, id, keyVersion);
    return xchacha20poly1305Open(
      this.#keys.key(keyVersion),
      envelope.slice(1, 25),
      aad,
      envelope.slice(25),
    );
  }

  async digest(bytes: Uint8Array): Promise<Uint8Array> {
    return new Uint8Array(await this.#crypto.subtle.digest("SHA-256", Uint8Array.from(bytes).buffer));
  }

  async verifyEd25519(publicKey: Uint8Array, message: Uint8Array, signature: Uint8Array): Promise<boolean> {
    exactBytes(publicKey, 32, "provider acknowledgement public key");
    exactBytes(signature, 64, "provider acknowledgement signature");
    try {
      const key = await this.#crypto.subtle.importKey("raw", Uint8Array.from(publicKey).buffer, "Ed25519", false, ["verify"]);
      return this.#crypto.subtle.verify("Ed25519", key, Uint8Array.from(signature).buffer, Uint8Array.from(message).buffer);
    } catch {
      return false;
    }
  }
}

export function copyContext(context: BrowserHostOutboxContextV1): BrowserHostOutboxContextV1 {
  return {
    profileId: context.profileId.slice(),
    registryHash: context.registryHash.slice(),
    genesisHash: context.genesisHash.slice(),
    negotiatedTuple: context.negotiatedTuple.slice(),
    providerId: context.providerId.slice(),
    providerEndpointHash: context.providerEndpointHash.slice(),
  };
}

export function outboxAad(context: BrowserHostOutboxContextV1, id: Uint8Array, keyVersion: number): Uint8Array {
  validateContext(context);
  exactBytes(id, 16, "outbox ID");
  return encodeHostV2Value({
    0: context.profileId,
    1: id,
    2: keyVersion,
    3: context.registryHash,
    4: context.genesisHash,
  });
}

function validateContext(context: BrowserHostOutboxContextV1): void {
  exactBytes(context.profileId, 32, "profile ID");
  exactBytes(context.registryHash, 32, "registry hash");
  exactBytes(context.genesisHash, 32, "genesis hash");
  exactBytes(context.negotiatedTuple, 32, "negotiated tuple");
  exactBytes(context.providerId, 32, "provider ID");
  exactBytes(context.providerEndpointHash, 32, "provider endpoint hash");
}

function exactBytes(value: Uint8Array, length: number, label: string): void {
  if (!(value instanceof Uint8Array) || value.length !== length) throw new TypeError(`${label} must be ${length} bytes`);
}

function read32(bytes: Uint8Array, offset: number): number {
  return (bytes[offset]! | bytes[offset + 1]! << 8 | bytes[offset + 2]! << 16 | bytes[offset + 3]! << 24) >>> 0;
}

function write32(bytes: Uint8Array, offset: number, value: number): void {
  bytes[offset] = value;
  bytes[offset + 1] = value >>> 8;
  bytes[offset + 2] = value >>> 16;
  bytes[offset + 3] = value >>> 24;
}

function rotate(value: number, amount: number): number {
  return ((value << amount) | (value >>> (32 - amount))) >>> 0;
}

function quarter(state: Uint32Array, a: number, b: number, c: number, d: number): void {
  state[a] = (state[a]! + state[b]!) >>> 0; state[d] = rotate(state[d]! ^ state[a]!, 16);
  state[c] = (state[c]! + state[d]!) >>> 0; state[b] = rotate(state[b]! ^ state[c]!, 12);
  state[a] = (state[a]! + state[b]!) >>> 0; state[d] = rotate(state[d]! ^ state[a]!, 8);
  state[c] = (state[c]! + state[d]!) >>> 0; state[b] = rotate(state[b]! ^ state[c]!, 7);
}

function rounds(state: Uint32Array): void {
  for (let round = 0; round < 10; round += 1) {
    quarter(state, 0, 4, 8, 12); quarter(state, 1, 5, 9, 13);
    quarter(state, 2, 6, 10, 14); quarter(state, 3, 7, 11, 15);
    quarter(state, 0, 5, 10, 15); quarter(state, 1, 6, 11, 12);
    quarter(state, 2, 7, 8, 13); quarter(state, 3, 4, 9, 14);
  }
}

function initialState(key: Uint8Array): Uint32Array {
  const state = new Uint32Array(16);
  state.set([0x61707865, 0x3320646e, 0x79622d32, 0x6b206574]);
  for (let index = 0; index < 8; index += 1) state[index + 4] = read32(key, index * 4);
  return state;
}

function hchacha20(key: Uint8Array, nonce: Uint8Array): Uint8Array {
  const state = initialState(key);
  for (let index = 0; index < 4; index += 1) state[index + 12] = read32(nonce, index * 4);
  rounds(state);
  const output = new Uint8Array(32);
  [0, 1, 2, 3, 12, 13, 14, 15].forEach((word, index) => write32(output, index * 4, state[word]!));
  return output;
}

function chachaBlock(key: Uint8Array, nonce: Uint8Array, counter: number): Uint8Array {
  const initial = initialState(key);
  initial[12] = counter >>> 0;
  initial[13] = read32(nonce, 0); initial[14] = read32(nonce, 4); initial[15] = read32(nonce, 8);
  const state = new Uint32Array(initial);
  rounds(state);
  const output = new Uint8Array(64);
  for (let index = 0; index < 16; index += 1) write32(output, index * 4, (state[index]! + initial[index]!) >>> 0);
  return output;
}

function streamXor(key: Uint8Array, nonce: Uint8Array, input: Uint8Array): Uint8Array {
  const output = new Uint8Array(input.length);
  for (let offset = 0, counter = 1; offset < input.length; offset += 64, counter += 1) {
    if (counter > 0xffff_ffff) throw new TypeError("XChaCha20 counter exhausted");
    const block = chachaBlock(key, nonce, counter);
    const length = Math.min(64, input.length - offset);
    for (let index = 0; index < length; index += 1) output[offset + index] = input[offset + index]! ^ block[index]!;
  }
  return output;
}

function littleInteger(bytes: Uint8Array): bigint {
  let value = 0n;
  for (let index = 0; index < bytes.length; index += 1) value |= BigInt(bytes[index]!) << BigInt(index * 8);
  return value;
}

function littleBytes(value: bigint, length: number): Uint8Array {
  const output = new Uint8Array(length);
  for (let index = 0; index < length; index += 1) output[index] = Number((value >> BigInt(index * 8)) & 0xffn);
  return output;
}

function poly1305(key: Uint8Array, message: Uint8Array): Uint8Array {
  const r = littleInteger(key.slice(0, 16)) & 0x0fff_fffc_0fff_fffc_0fff_fffc_0fff_ffffn;
  const s = littleInteger(key.slice(16, 32));
  let accumulator = 0n;
  for (let offset = 0; offset < message.length; offset += 16) {
    const block = message.slice(offset, Math.min(offset + 16, message.length));
    accumulator = ((accumulator + littleInteger(block) + (1n << BigInt(block.length * 8))) * r) % POLY_MODULUS;
  }
  return littleBytes((accumulator + s) & MASK_128, TAG_BYTES);
}

function pad16(length: number): number {
  return (16 - (length % 16)) % 16;
}

function length64(length: number): Uint8Array {
  if (!Number.isSafeInteger(length) || length < 0) throw new TypeError("AEAD length is invalid");
  return littleBytes(BigInt(length), 8);
}

function macData(aad: Uint8Array, ciphertext: Uint8Array): Uint8Array {
  const output = new Uint8Array(aad.length + pad16(aad.length) + ciphertext.length + pad16(ciphertext.length) + 16);
  let offset = 0;
  output.set(aad, offset); offset += aad.length + pad16(aad.length);
  output.set(ciphertext, offset); offset += ciphertext.length + pad16(ciphertext.length);
  output.set(length64(aad.length), offset); output.set(length64(ciphertext.length), offset + 8);
  return output;
}

function xnonce(key: Uint8Array, nonce: Uint8Array): { readonly key: Uint8Array; readonly nonce: Uint8Array } {
  exactBytes(key, 32, "XChaCha20 key"); exactBytes(nonce, 24, "XChaCha20 nonce");
  const derived = hchacha20(key, nonce.slice(0, 16));
  const chachaNonce = new Uint8Array(12); chachaNonce.set(nonce.slice(16), 4);
  return { key: derived, nonce: chachaNonce };
}

function xchacha20poly1305Seal(key: Uint8Array, nonce: Uint8Array, aad: Uint8Array, plaintext: Uint8Array): Uint8Array {
  const material = xnonce(key, nonce);
  const ciphertext = streamXor(material.key, material.nonce, plaintext);
  const oneTimeKey = chachaBlock(material.key, material.nonce, 0).slice(0, 32);
  const tag = poly1305(oneTimeKey, macData(aad, ciphertext));
  const output = new Uint8Array(ciphertext.length + TAG_BYTES);
  output.set(ciphertext); output.set(tag, ciphertext.length);
  return output;
}

function xchacha20poly1305Open(key: Uint8Array, nonce: Uint8Array, aad: Uint8Array, sealed: Uint8Array): Uint8Array {
  if (sealed.length < TAG_BYTES) throw new TypeError("XChaCha20-Poly1305 ciphertext is truncated");
  const material = xnonce(key, nonce);
  const ciphertext = sealed.slice(0, -TAG_BYTES);
  const actual = sealed.slice(-TAG_BYTES);
  const expected = poly1305(chachaBlock(material.key, material.nonce, 0).slice(0, 32), macData(aad, ciphertext));
  let difference = 0;
  for (let index = 0; index < TAG_BYTES; index += 1) difference |= actual[index]! ^ expected[index]!;
  if (difference !== 0) throw new TypeError("XChaCha20-Poly1305 authentication failed");
  return streamXor(material.key, material.nonce, ciphertext);
}
