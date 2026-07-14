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

import { NativeDomainError, type JsonObject } from "./errors.ts";
import { blake2b256 } from "@cord-network/origin-sdk-crypto";

export type ContentCodec = "raw" | "dag-pb";
export type ContentMultihash = "blake2b-256" | "sha2-256";

/** A CID plus the codec and hash declaration made by the calling application. */
export interface ContentAddress {
  readonly cid: string;
  readonly codec: ContentCodec;
  readonly multihash: ContentMultihash;
}

export interface ParsedContentCid {
  readonly version: 0 | 1;
  readonly codec: ContentCodec;
  readonly multihash: ContentMultihash;
  readonly digest: Uint8Array;
}

export type ContentBody = Uint8Array | ArrayBuffer | AsyncIterable<Uint8Array>;

export interface ContentFetchRequest {
  readonly cid: string;
  readonly signal: AbortSignal | undefined;
  /** The source must not intentionally buffer more than this many bytes. The SDK enforces it too. */
  readonly maxBytes: number;
}

/** Caller-injected block source. Array order is the deterministic failover order. */
export interface ContentFetchProvider {
  readonly id: string;
  readonly kind: "gateway" | "bitswap" | (string & {});
  fetchBlock(request: ContentFetchRequest): Promise<ContentBody>;
}

export type GatewayTransport = (
  locator: string,
  request: ContentFetchRequest,
) => Promise<ContentBody>;
export type BitswapTransport = (request: ContentFetchRequest) => Promise<ContentBody>;

export interface DagPbDecodeContext {
  /** CID-verified DAG-PB root block. It is not decoded or represented as file bytes by the SDK. */
  readonly root: Uint8Array;
  readonly rootCid: string;
  /** Every loaded linked block is CID-verified before it reaches the decoder. */
  readonly loadBlock: (cid: string) => Promise<Uint8Array>;
  readonly signal: AbortSignal | undefined;
  readonly maxContentBytes: number;
}

/** Inject an audited DAG-PB/UnixFS decoder; this module deliberately does not pretend raw DAG bytes are a file. */
export interface DagPbContentDecoder {
  decode(context: DagPbDecodeContext): Promise<ContentBody>;
}

export interface ContentClientOptions {
  readonly providers: readonly ContentFetchProvider[];
  readonly dagPbDecoder?: DagPbContentDecoder;
  readonly maxBlockBytes?: number;
  readonly maxContentBytes?: number;
  readonly maxBlocks?: number;
}

export interface ContentFetchOptions {
  readonly signal?: AbortSignal;
}

const CODECS: Record<ContentCodec, number> = { raw: 0x55, "dag-pb": 0x70 };
const MULTIHASHES: Record<ContentMultihash, number> = { "sha2-256": 0x12, "blake2b-256": 0xb220 };
const DEFAULT_MAX_BLOCK_BYTES = 4 * 1024 * 1024;
const DEFAULT_MAX_CONTENT_BYTES = 64 * 1024 * 1024;
const DEFAULT_MAX_BLOCKS = 4_096;

function contentError(
  operation: string,
  code: "content_unavailable" | "content_integrity" | "invalid_input",
  message: string,
  retryable = false,
  details: JsonObject = {},
): NativeDomainError {
  return new NativeDomainError("content", operation, code, message, retryable, details);
}

function assertBound(value: number, label: string): void {
  if (!Number.isSafeInteger(value) || value < 1) {
    throw contentError("configure", "invalid_input", `${label} must be a positive safe integer`);
  }
}

function cancelled(operation: string): NativeDomainError {
  return contentError(operation, "content_unavailable", "content fetch cancelled", false, {
    cancelled: true,
  });
}

function throwIfAborted(signal: AbortSignal | undefined, operation: string): void {
  if (signal?.aborted) throw cancelled(operation);
}

async function abortable<T>(
  promise: Promise<T>,
  signal: AbortSignal | undefined,
  operation: string,
): Promise<T> {
  if (!signal) return promise;
  throwIfAborted(signal, operation);
  let onAbort: (() => void) | undefined;
  const aborted = new Promise<never>((_, reject) => {
    onAbort = () => reject(cancelled(operation));
    signal.addEventListener("abort", onAbort, { once: true });
  });
  try {
    return await Promise.race([promise, aborted]);
  } finally {
    if (onAbort) signal.removeEventListener("abort", onAbort);
  }
}

function isAsyncIterable(value: unknown): value is AsyncIterable<Uint8Array> {
  return !!value && typeof (value as { [Symbol.asyncIterator]?: unknown })[Symbol.asyncIterator] === "function";
}

async function collectBody(
  body: ContentBody,
  maxBytes: number,
  signal: AbortSignal | undefined,
  operation: string,
): Promise<Uint8Array> {
  throwIfAborted(signal, operation);
  if (body instanceof Uint8Array) {
    if (body.byteLength > maxBytes) {
      throw contentError(operation, "content_unavailable", "content exceeds the configured size bound", false, {
        max_bytes: maxBytes,
      });
    }
    return body.slice();
  }
  if (body instanceof ArrayBuffer) {
    if (body.byteLength > maxBytes) {
      throw contentError(operation, "content_unavailable", "content exceeds the configured size bound", false, {
        max_bytes: maxBytes,
      });
    }
    return new Uint8Array(body.slice(0));
  }
  if (!isAsyncIterable(body)) {
    throw contentError(operation, "content_unavailable", "content source returned an unsupported body", true);
  }
  const chunks: Uint8Array[] = [];
  let length = 0;
  const iterator = body[Symbol.asyncIterator]();
  try {
    while (true) {
      const next = await abortable(Promise.resolve(iterator.next()), signal, operation);
      if (next.done) break;
      const chunk = next.value;
      if (!(chunk instanceof Uint8Array)) {
        throw contentError(operation, "content_unavailable", "content source yielded a non-byte chunk", true);
      }
      length += chunk.byteLength;
      if (length > maxBytes) {
        throw contentError(operation, "content_unavailable", "content exceeds the configured size bound", false, {
          max_bytes: maxBytes,
        });
      }
      chunks.push(chunk.slice());
    }
  } finally {
    if (signal?.aborted && iterator.return) {
      try {
        void Promise.resolve(iterator.return()).catch(() => {});
      } catch {
        // Cancellation already has a single typed terminal result; late source cleanup is best effort.
      }
    }
  }
  const result = new Uint8Array(length);
  let offset = 0;
  for (const chunk of chunks) {
    result.set(chunk, offset);
    offset += chunk.byteLength;
  }
  return result;
}

const BASE32 = "abcdefghijklmnopqrstuvwxyz234567";
function encodeBase32(bytes: Uint8Array): string {
  let accumulator = 0;
  let bits = 0;
  let encoded = "";
  for (const byte of bytes) {
    accumulator = accumulator * 256 + byte;
    bits += 8;
    while (bits >= 5) {
      const divisor = 2 ** (bits - 5);
      encoded += BASE32[Math.floor(accumulator / divisor) & 31];
      accumulator %= divisor;
      bits -= 5;
    }
  }
  if (bits) encoded += BASE32[(accumulator * 2 ** (5 - bits)) & 31];
  return encoded;
}

function decodeBase32(value: string): Uint8Array {
  if (!value) throw new Error("empty base32 CID");
  const bytes: number[] = [];
  let accumulator = 0;
  let bits = 0;
  for (const character of value.toLowerCase()) {
    const digit = BASE32.indexOf(character);
    if (digit < 0) throw new Error("invalid base32 CID");
    accumulator = accumulator * 32 + digit;
    bits += 5;
    while (bits >= 8) {
      const divisor = 2 ** (bits - 8);
      bytes.push(Math.floor(accumulator / divisor) & 0xff);
      accumulator %= divisor;
      bits -= 8;
    }
  }
  if (accumulator !== 0) throw new Error("non-canonical base32 CID");
  return Uint8Array.from(bytes);
}

const BASE58 = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
function encodeBase58(bytes: Uint8Array): string {
  let value = 0n;
  for (const byte of bytes) value = value * 256n + BigInt(byte);
  let encoded = "";
  while (value > 0n) {
    encoded = BASE58[Number(value % 58n)] + encoded;
    value /= 58n;
  }
  let leadingZeroes = 0;
  while (bytes[leadingZeroes] === 0) leadingZeroes += 1;
  return "1".repeat(leadingZeroes) + encoded;
}

function decodeBase58(value: string): Uint8Array {
  if (!value) throw new Error("empty base58 CID");
  let decoded = 0n;
  for (const character of value) {
    const digit = BASE58.indexOf(character);
    if (digit < 0) throw new Error("invalid base58 CID");
    decoded = decoded * 58n + BigInt(digit);
  }
  const suffix: number[] = [];
  while (decoded > 0n) {
    suffix.push(Number(decoded & 0xffn));
    decoded >>= 8n;
  }
  suffix.reverse();
  let leadingZeroes = 0;
  while (value[leadingZeroes] === "1") leadingZeroes += 1;
  return Uint8Array.from([...new Array(leadingZeroes).fill(0), ...suffix]);
}

function encodeVarint(value: number): number[] {
  const encoded: number[] = [];
  let remaining = value;
  do {
    let byte = remaining % 128;
    remaining = Math.floor(remaining / 128);
    if (remaining > 0) byte |= 0x80;
    encoded.push(byte);
  } while (remaining > 0);
  return encoded;
}

function readVarint(bytes: Uint8Array, start: number): { value: number; next: number } {
  let value = 0;
  let factor = 1;
  for (let offset = start; offset < bytes.length && offset < start + 9; offset += 1) {
    const byte = bytes[offset];
    value += (byte & 0x7f) * factor;
    if (!Number.isSafeInteger(value)) throw new Error("CID varint is too large");
    if ((byte & 0x80) === 0) {
      const length = offset - start + 1;
      if (encodeVarint(value).length !== length) throw new Error("non-canonical CID varint");
      return { value, next: offset + 1 };
    }
    factor *= 128;
  }
  throw new Error("unterminated CID varint");
}

function codecName(value: number): ContentCodec {
  if (value === CODECS.raw) return "raw";
  if (value === CODECS["dag-pb"]) return "dag-pb";
  throw new Error(`unsupported multicodec ${value}`);
}

function multihashName(value: number): ContentMultihash {
  if (value === MULTIHASHES["sha2-256"]) return "sha2-256";
  if (value === MULTIHASHES["blake2b-256"]) return "blake2b-256";
  throw new Error(`unsupported multihash ${value}`);
}

function parseMultihash(bytes: Uint8Array, start: number): {
  readonly multihash: ContentMultihash;
  readonly digest: Uint8Array;
} {
  const code = readVarint(bytes, start);
  const length = readVarint(bytes, code.next);
  if (length.value !== 32 || length.next + length.value !== bytes.length) {
    throw new Error("CID must contain one exact 32-byte multihash digest");
  }
  return { multihash: multihashName(code.value), digest: bytes.slice(length.next) };
}

/** Parse only the supported, integrity-verifiable CID subset. */
export function parseContentCid(cid: string): ParsedContentCid {
  try {
    if (!cid || cid.length > 256 || cid.trim() !== cid) throw new Error("CID must contain 1-256 characters");
    if (cid.startsWith("Qm")) {
      const bytes = decodeBase58(cid);
      if (encodeBase58(bytes) !== cid) throw new Error("non-canonical base58 CID");
      const hash = parseMultihash(bytes, 0);
      if (hash.multihash !== "sha2-256") throw new Error("CIDv0 must use sha2-256");
      return { version: 0, codec: "dag-pb", ...hash };
    }
    let bytes: Uint8Array;
    if (cid[0] === "b" || cid[0] === "B") {
      const payload = cid.slice(1);
      bytes = decodeBase32(payload);
      const canonical = encodeBase32(bytes);
      if (payload !== (cid[0] === "B" ? canonical.toUpperCase() : canonical)) {
        throw new Error("non-canonical base32 CID");
      }
    } else if (cid[0] === "z") {
      const payload = cid.slice(1);
      bytes = decodeBase58(payload);
      if (encodeBase58(bytes) !== payload) throw new Error("non-canonical base58 CID");
    }
    else throw new Error("CID must use base32 or base58btc multibase");
    const version = readVarint(bytes, 0);
    if (version.value !== 1) throw new Error("only CIDv0 and CIDv1 are supported");
    const codec = readVarint(bytes, version.next);
    const hash = parseMultihash(bytes, codec.next);
    return { version: 1, codec: codecName(codec.value), ...hash };
  } catch (error) {
    if (error instanceof NativeDomainError) throw error;
    throw contentError("cid.parse", "content_integrity", error instanceof Error ? error.message : "invalid CID");
  }
}

export function declareContentAddress(
  cid: string,
  codec: ContentCodec,
  multihash: ContentMultihash,
): ContentAddress {
  const parsed = parseContentCid(cid);
  if (parsed.codec !== codec || parsed.multihash !== multihash) {
    throw contentError("cid.declare", "content_integrity", "CID does not match its declared codec and multihash", false, {
      declared_codec: codec,
      declared_multihash: multihash,
      cid_codec: parsed.codec,
      cid_multihash: parsed.multihash,
    });
  }
  return { cid, codec, multihash };
}

const SHA256_K = Uint32Array.from([
  0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
  0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
  0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
  0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
  0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
  0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
  0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
  0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
]);

function rotateRight32(value: number, amount: number): number {
  return (value >>> amount) | (value << (32 - amount));
}

function sha256(input: Uint8Array): Uint8Array {
  const bitLength = BigInt(input.byteLength) * 8n;
  const paddedLength = Math.ceil((input.byteLength + 9) / 64) * 64;
  const padded = new Uint8Array(paddedLength);
  padded.set(input);
  padded[input.byteLength] = 0x80;
  for (let index = 0; index < 8; index += 1) {
    padded[paddedLength - 1 - index] = Number((bitLength >> BigInt(index * 8)) & 0xffn);
  }
  const hash = Uint32Array.from([
    0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
    0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
  ]);
  const schedule = new Uint32Array(64);
  for (let offset = 0; offset < padded.length; offset += 64) {
    for (let index = 0; index < 16; index += 1) {
      const position = offset + index * 4;
      schedule[index] = (
        (padded[position] << 24) | (padded[position + 1] << 16) |
        (padded[position + 2] << 8) | padded[position + 3]
      ) >>> 0;
    }
    for (let index = 16; index < 64; index += 1) {
      const x = schedule[index - 15];
      const y = schedule[index - 2];
      const s0 = rotateRight32(x, 7) ^ rotateRight32(x, 18) ^ (x >>> 3);
      const s1 = rotateRight32(y, 17) ^ rotateRight32(y, 19) ^ (y >>> 10);
      schedule[index] = (schedule[index - 16] + s0 + schedule[index - 7] + s1) >>> 0;
    }
    let [a, b, c, d, e, f, g, h] = hash;
    for (let index = 0; index < 64; index += 1) {
      const upper = rotateRight32(e, 6) ^ rotateRight32(e, 11) ^ rotateRight32(e, 25);
      const choice = (e & f) ^ (~e & g);
      const t1 = (h + upper + choice + SHA256_K[index] + schedule[index]) >>> 0;
      const lower = rotateRight32(a, 2) ^ rotateRight32(a, 13) ^ rotateRight32(a, 22);
      const majority = (a & b) ^ (a & c) ^ (b & c);
      const t2 = (lower + majority) >>> 0;
      h = g; g = f; f = e; e = (d + t1) >>> 0; d = c; c = b; b = a; a = (t1 + t2) >>> 0;
    }
    hash[0] = (hash[0] + a) >>> 0; hash[1] = (hash[1] + b) >>> 0;
    hash[2] = (hash[2] + c) >>> 0; hash[3] = (hash[3] + d) >>> 0;
    hash[4] = (hash[4] + e) >>> 0; hash[5] = (hash[5] + f) >>> 0;
    hash[6] = (hash[6] + g) >>> 0; hash[7] = (hash[7] + h) >>> 0;
  }
  const output = new Uint8Array(32);
  for (let index = 0; index < hash.length; index += 1) {
    output[index * 4] = hash[index] >>> 24;
    output[index * 4 + 1] = hash[index] >>> 16;
    output[index * 4 + 2] = hash[index] >>> 8;
    output[index * 4 + 3] = hash[index];
  }
  return output;
}

export function digestContent(multihash: ContentMultihash, bytes: Uint8Array): Uint8Array {
  return multihash === "sha2-256" ? sha256(bytes) : blake2b256(bytes);
}

function equalBytes(left: Uint8Array, right: Uint8Array): boolean {
  if (left.byteLength !== right.byteLength) return false;
  let difference = 0;
  for (let index = 0; index < left.byteLength; index += 1) difference |= left[index] ^ right[index];
  return difference === 0;
}

export function verifyContentBlock(cid: string, bytes: Uint8Array): ParsedContentCid {
  const parsed = parseContentCid(cid);
  if (!equalBytes(parsed.digest, digestContent(parsed.multihash, bytes))) {
    throw contentError("cid.verify", "content_integrity", "content bytes do not match the CID multihash", false, {
      cid,
      codec: parsed.codec,
      multihash: parsed.multihash,
    });
  }
  return parsed;
}

function validateProviderId(id: string): void {
  if (!id || id.length > 128) {
    throw contentError("configure", "invalid_input", "provider id must contain 1-128 characters");
  }
}

export function gatewayContentProvider(input: {
  readonly id: string;
  readonly baseUrl: string;
  readonly transport: GatewayTransport;
}): ContentFetchProvider {
  validateProviderId(input.id);
  let base: URL;
  try {
    base = new URL(input.baseUrl);
    if (
      (base.protocol !== "http:" && base.protocol !== "https:") ||
      base.username || base.password || base.search || base.hash
    ) throw new Error("not a clean HTTP gateway base");
    if (!base.pathname.endsWith("/")) base.pathname += "/";
  } catch {
    throw contentError(
      "configure",
      "invalid_input",
      "gateway baseUrl must be an absolute HTTP(S) URL without credentials, query or fragment",
    );
  }
  return {
    id: input.id,
    kind: "gateway",
    fetchBlock(request) {
      const locator = new URL(encodeURIComponent(request.cid), base).toString();
      return input.transport(locator, request);
    },
  };
}

export function bitswapContentProvider(input: {
  readonly id: string;
  readonly transport: BitswapTransport;
}): ContentFetchProvider {
  validateProviderId(input.id);
  return { id: input.id, kind: "bitswap", fetchBlock: input.transport };
}

export class ContentClient {
  private readonly providers: readonly ContentFetchProvider[];
  private readonly decoder: DagPbContentDecoder | undefined;
  private readonly maxBlockBytes: number;
  private readonly maxContentBytes: number;
  private readonly maxBlocks: number;

  constructor(options: ContentClientOptions) {
    if (!options.providers.length) {
      throw contentError("configure", "invalid_input", "at least one content provider is required");
    }
    const ids = new Set<string>();
    for (const provider of options.providers) {
      validateProviderId(provider.id);
      if (ids.has(provider.id)) throw contentError("configure", "invalid_input", "provider ids must be unique");
      ids.add(provider.id);
    }
    this.maxBlockBytes = options.maxBlockBytes ?? DEFAULT_MAX_BLOCK_BYTES;
    this.maxContentBytes = options.maxContentBytes ?? DEFAULT_MAX_CONTENT_BYTES;
    this.maxBlocks = options.maxBlocks ?? DEFAULT_MAX_BLOCKS;
    assertBound(this.maxBlockBytes, "maxBlockBytes");
    assertBound(this.maxContentBytes, "maxContentBytes");
    assertBound(this.maxBlocks, "maxBlocks");
    this.providers = [...options.providers];
    this.decoder = options.dagPbDecoder;
  }

  private async fetchVerifiedBlock(cid: string, signal: AbortSignal | undefined): Promise<Uint8Array> {
    const parsed = parseContentCid(cid);
    const attempts: string[] = [];
    let sawIntegrityFailure = false;
    for (const provider of this.providers) {
      throwIfAborted(signal, "fetch");
      try {
        const body = await abortable(
          provider.fetchBlock({ cid, signal, maxBytes: this.maxBlockBytes }),
          signal,
          "fetch",
        );
        const bytes = await collectBody(body, this.maxBlockBytes, signal, "fetch");
        if (!equalBytes(parsed.digest, digestContent(parsed.multihash, bytes))) {
          sawIntegrityFailure = true;
          attempts.push(`${provider.id}:content_integrity`);
          continue;
        }
        return bytes;
      } catch (error) {
        if (signal?.aborted) throw cancelled("fetch");
        if (error instanceof NativeDomainError && error.code === "content_integrity") {
          sawIntegrityFailure = true;
          attempts.push(`${provider.id}:content_integrity`);
        } else {
          attempts.push(`${provider.id}:content_unavailable`);
        }
      }
    }
    if (sawIntegrityFailure) {
      throw contentError("fetch", "content_integrity", "no provider returned bytes matching the CID", false, {
        cid,
        attempts,
      });
    }
    throw contentError("fetch", "content_unavailable", "content is unavailable from every configured provider", true, {
      cid,
      attempts,
    });
  }

  /** Fetch raw content or reconstruct DAG-PB/UnixFS through the explicitly injected decoder. */
  async fetch(address: ContentAddress, options: ContentFetchOptions = {}): Promise<Uint8Array> {
    if (
      !address || typeof address.cid !== "string" ||
      (address.codec !== "raw" && address.codec !== "dag-pb") ||
      (address.multihash !== "sha2-256" && address.multihash !== "blake2b-256")
    ) {
      throw contentError("fetch", "invalid_input", "content address must declare a supported CID, codec and multihash");
    }
    const parsed = parseContentCid(address.cid);
    if (parsed.codec !== address.codec || parsed.multihash !== address.multihash) {
      throw contentError("fetch", "content_integrity", "CID does not match its declared codec and multihash", false, {
        declared_codec: address.codec,
        declared_multihash: address.multihash,
        cid_codec: parsed.codec,
        cid_multihash: parsed.multihash,
      });
    }
    const cache = new Map<string, Promise<Uint8Array>>();
    let loadedBlocks = 0;
    const loadBlock = (cid: string): Promise<Uint8Array> => {
      const existing = cache.get(cid);
      if (existing) return existing;
      if (loadedBlocks >= this.maxBlocks) {
        throw contentError("fetch", "content_unavailable", "DAG exceeds the configured block-count bound", false, {
          max_blocks: this.maxBlocks,
        });
      }
      loadedBlocks += 1;
      const pending = this.fetchVerifiedBlock(cid, options.signal);
      cache.set(cid, pending);
      return pending;
    };
    const root = await loadBlock(address.cid);
    if (parsed.codec === "raw") {
      if (root.byteLength > this.maxContentBytes) {
        throw contentError("fetch", "content_unavailable", "content exceeds the configured size bound", false, {
          max_bytes: this.maxContentBytes,
        });
      }
      return root;
    }
    if (!this.decoder) {
      throw contentError(
        "fetch",
        "content_unavailable",
        "DAG-PB/UnixFS content requires an injected verified-block decoder",
        false,
        { decoder_required: true, codec: "dag-pb" },
      );
    }
    let decoded: ContentBody;
    try {
      decoded = await abortable(
        this.decoder.decode({
          root,
          rootCid: address.cid,
          loadBlock,
          signal: options.signal,
          maxContentBytes: this.maxContentBytes,
        }),
        options.signal,
        "fetch",
      );
    } catch (error) {
      if (error instanceof NativeDomainError) throw error;
      throw contentError("fetch", "content_unavailable", "DAG-PB/UnixFS decoder failed", false, {
        decoder_failed: true,
      });
    }
    return collectBody(decoded, this.maxContentBytes, options.signal, "fetch");
  }
}

export function createContentClient(options: ContentClientOptions): ContentClient {
  return new ContentClient(options);
}
