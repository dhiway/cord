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

import {
  HOST_V2_CROSS_FIELD_RULES,
  HOST_V2_SCHEMAS,
  type HostV2SchemaNode,
  type HostV2TypeMap,
  type HostV2TypeName,
} from "./generated.ts";

export interface HostV2Map {
  readonly [key: number]: HostV2Value;
}

export type HostV2Value =
  | boolean
  | number
  | bigint
  | string
  | Uint8Array
  | readonly HostV2Value[]
  | HostV2Map;

export class HostV2CodecError extends Error {
  readonly code: "WIRE_SCHEMA_INVALID" | "WIRE_NON_CANONICAL";

  constructor(code: HostV2CodecError["code"], message: string) {
    super(message);
    this.name = "HostV2CodecError";
    this.code = code;
  }
}

export interface HostV2Dto<Name extends HostV2TypeName> {
  readonly production: Name;
  readonly value: HostV2TypeMap[Name];
  readonly canonicalBytes: Uint8Array;
}

const utf8 = new TextEncoder();
const utf8Decoder = new TextDecoder("utf-8", { fatal: true });
const MAX_U64 = 0xffff_ffff_ffff_ffffn;

function utf8Bytes(value: string, label: string): Uint8Array {
  for (let index = 0; index < value.length; index += 1) {
    const unit = value.charCodeAt(index);
    if (unit >= 0xd800 && unit <= 0xdbff) {
      const next = value.charCodeAt(index + 1);
      if (!(next >= 0xdc00 && next <= 0xdfff)) failSchema(`${label} contains an unpaired UTF-16 surrogate`);
      index += 1;
    } else if (unit >= 0xdc00 && unit <= 0xdfff) {
      failSchema(`${label} contains an unpaired UTF-16 surrogate`);
    }
  }
  return utf8.encode(value);
}

function failSchema(message: string): never {
  throw new HostV2CodecError("WIRE_SCHEMA_INVALID", message);
}

function failNonCanonical(message: string): never {
  throw new HostV2CodecError("WIRE_NON_CANONICAL", message);
}

function asUnsigned(value: number | bigint, label: string): bigint {
  if (typeof value === "number") {
    if (!Number.isSafeInteger(value) || value < 0) failSchema(`${label} must be an unsigned integer`);
    return BigInt(value);
  }
  if (value < 0n || value > MAX_U64) failSchema(`${label} must fit u64`);
  return value;
}

function encodeHead(major: number, value: bigint): Uint8Array {
  if (value <= 23n) return Uint8Array.of((major << 5) | Number(value));
  if (value <= 0xffn) return Uint8Array.of((major << 5) | 24, Number(value));
  if (value <= 0xffffn) return Uint8Array.of(
    (major << 5) | 25,
    Number(value >> 8n),
    Number(value & 0xffn),
  );
  if (value <= 0xffff_ffffn) return Uint8Array.of(
    (major << 5) | 26,
    Number((value >> 24n) & 0xffn),
    Number((value >> 16n) & 0xffn),
    Number((value >> 8n) & 0xffn),
    Number(value & 0xffn),
  );
  if (value <= MAX_U64) return Uint8Array.of(
    (major << 5) | 27,
    ...Array.from({ length: 8 }, (_, index) => Number((value >> BigInt((7 - index) * 8)) & 0xffn)),
  );
  return failSchema("CBOR unsigned integer exceeds u64");
}

function concat(parts: readonly Uint8Array[]): Uint8Array {
  const output = new Uint8Array(parts.reduce((length, part) => length + part.length, 0));
  let offset = 0;
  for (const part of parts) {
    output.set(part, offset);
    offset += part.length;
  }
  return output;
}

function isMap(value: HostV2Value): value is HostV2Map {
  return typeof value === "object"
    && value !== null
    && !(value instanceof Uint8Array)
    && !Array.isArray(value);
}

function mapKeys(value: HostV2Map): number[] {
  return Object.keys(value).map((key) => {
    if (!/^(0|[1-9][0-9]*)$/.test(key)) failSchema("CBOR map keys must be canonical unsigned integers");
    const parsed = Number(key);
    if (!Number.isSafeInteger(parsed)) failSchema("CBOR map key exceeds safe integer range");
    return parsed;
  });
}

export function encodeHostV2Value(value: HostV2Value): Uint8Array {
  if (typeof value === "boolean") return Uint8Array.of(value ? 0xf5 : 0xf4);
  if (typeof value === "number" || typeof value === "bigint") return encodeHead(0, asUnsigned(value, "value"));
  if (typeof value === "string") {
    if (value.normalize("NFC") !== value) failSchema("text must already be NFC");
    const bytes = utf8Bytes(value, "text");
    return concat([encodeHead(3, BigInt(bytes.length)), bytes]);
  }
  if (value instanceof Uint8Array) return concat([encodeHead(2, BigInt(value.length)), value]);
  if (Array.isArray(value)) {
    return concat([encodeHead(4, BigInt(value.length)), ...value.map(encodeHostV2Value)]);
  }
  if (isMap(value)) {
    const entries = mapKeys(value).map((key) => {
      const encodedKey = encodeHead(0, BigInt(key));
      return { encodedKey, encodedValue: encodeHostV2Value(value[key]!) };
    }).sort((left, right) => left.encodedKey.length - right.encodedKey.length
      || compareBytes(left.encodedKey, right.encodedKey));
    return concat([
      encodeHead(5, BigInt(entries.length)),
      ...entries.flatMap(({ encodedKey, encodedValue }) => [encodedKey, encodedValue]),
    ]);
  }
  return failSchema("unsupported host-v2 value");
}

function compareBytes(left: Uint8Array, right: Uint8Array): number {
  const length = Math.min(left.length, right.length);
  for (let index = 0; index < length; index += 1) {
    const delta = left[index]! - right[index]!;
    if (delta !== 0) return delta;
  }
  return left.length - right.length;
}

class Decoder {
  private offset = 0;
  private readonly source: Uint8Array;

  constructor(source: Uint8Array) {
    this.source = source;
  }

  decode(): HostV2Value {
    const value = this.item();
    if (this.offset !== this.source.length) failSchema("trailing bytes after CBOR value");
    return value;
  }

  private byte(): number {
    const value = this.source[this.offset];
    if (value === undefined) failSchema("truncated CBOR value");
    this.offset += 1;
    return value;
  }

  private argument(additional: number): bigint {
    if (additional < 24) return BigInt(additional);
    const bytes = additional === 24 ? 1 : additional === 25 ? 2 : additional === 26 ? 4 : additional === 27 ? 8 : 0;
    if (additional === 31) failNonCanonical("indefinite-length CBOR is forbidden");
    if (bytes === 0) failSchema("reserved CBOR argument is forbidden");
    let value = 0n;
    for (let index = 0; index < bytes; index += 1) value = (value << 8n) | BigInt(this.byte());
    return value;
  }

  private boundedLength(value: bigint): number {
    if (value > BigInt(Number.MAX_SAFE_INTEGER)) failSchema("CBOR length exceeds safe range");
    return Number(value);
  }

  private bytes(length: number): Uint8Array {
    if (this.offset + length > this.source.length) failSchema("truncated CBOR bytes");
    const value = this.source.slice(this.offset, this.offset + length);
    this.offset += length;
    return value;
  }

  private item(): HostV2Value {
    const initial = this.byte();
    const major = initial >> 5;
    const additional = initial & 31;
    if (major === 7) {
      if (additional === 20) return false;
      if (additional === 21) return true;
      return failSchema("floats, null, undefined, and simple values are forbidden");
    }
    if (major === 6) return failNonCanonical("CBOR tags are forbidden");
    if (major === 1) return failSchema("negative integers are forbidden");
    const argument = this.argument(additional);
    if (major === 0) return argument <= BigInt(Number.MAX_SAFE_INTEGER) ? Number(argument) : argument;
    if (major === 2) return this.bytes(this.boundedLength(argument));
    if (major === 3) {
      try {
        const value = utf8Decoder.decode(this.bytes(this.boundedLength(argument)));
        if (value.normalize("NFC") !== value) failSchema("text must already be NFC");
        return value;
      } catch (error) {
        if (error instanceof HostV2CodecError) throw error;
        return failSchema("invalid UTF-8 text");
      }
    }
    if (major === 4) {
      return Array.from({ length: this.boundedLength(argument) }, () => this.item());
    }
    if (major === 5) {
      const output: Record<number, HostV2Value> = Object.create(null) as Record<number, HostV2Value>;
      for (let index = 0; index < this.boundedLength(argument); index += 1) {
        const key = this.item();
        if (typeof key !== "number" || !Number.isSafeInteger(key) || key < 0) failSchema("map key must be a u64-safe integer");
        if (Object.hasOwn(output, key)) failSchema("duplicate map key");
        output[key] = this.item();
      }
      return output;
    }
    return failSchema("unsupported CBOR major type");
  }
}

export function decodeCanonicalHostV2Value(bytes: Uint8Array): HostV2Value {
  const value = new Decoder(bytes).decode();
  const canonical = encodeHostV2Value(value);
  if (compareBytes(canonical, bytes) !== 0 || canonical.length !== bytes.length) {
    throw new HostV2CodecError("WIRE_NON_CANONICAL", "host-v2 CBOR is not deterministic canonical encoding");
  }
  return value;
}

function integerValue(value: HostV2Value, path: string): bigint {
  if (typeof value !== "number" && typeof value !== "bigint") return failSchema(`${path} must be an unsigned integer`);
  return asUnsigned(value, path);
}

function validateNode(node: HostV2SchemaNode, value: HostV2Value, path: string): void {
  if (node.kind === "ref") return validateNode(HOST_V2_SCHEMAS[node.name], value, `${path}:${node.name}`);
  if (node.kind === "union") {
    for (const variant of node.variants) {
      try { validateNode(variant, value, path); return; } catch (error) {
        if (!(error instanceof HostV2CodecError)) throw error;
      }
    }
    return failSchema(`${path} matches no closed union variant`);
  }
  if (node.kind === "map") {
    if (!isMap(value)) return failSchema(`${path} must be a closed map`);
    const keys = mapKeys(value);
    const allowed = new Set(node.fields.map((field) => field.key));
    if (keys.some((key) => !allowed.has(key))) return failSchema(`${path} contains an unknown field`);
    for (const field of node.fields) {
      if (!Object.prototype.hasOwnProperty.call(value, field.key)) {
        if (field.required) failSchema(`${path} is missing field ${field.key}`);
        continue;
      }
      validateNode(field.schema, value[field.key]!, `${path}/${field.key}`);
    }
    return;
  }
  if (node.kind === "array") {
    if (!Array.isArray(value) || value.length < node.min || value.length > node.max) {
      return failSchema(`${path} violates array bounds`);
    }
    value.forEach((item, index) => validateNode(node.items, item, `${path}/${index}`));
    return;
  }
  if (node.kind === "uint") {
    const integer = integerValue(value, path);
    if (integer < BigInt(node.min) || integer > BigInt(node.max)) failSchema(`${path} violates integer bounds`);
    return;
  }
  if (node.kind === "bytes") {
    if (!(value instanceof Uint8Array) || value.length < node.min || value.length > node.max) {
      return failSchema(`${path} violates byte bounds`);
    }
    return;
  }
  if (node.kind === "text") {
    if (typeof value !== "string") return failSchema(`${path} must be text`);
    const length = utf8Bytes(value, path).length;
    if (length < node.min || length > node.max || (node.nfc === true && value.normalize("NFC") !== value)) {
      return failSchema(`${path} violates text bounds`);
    }
    return;
  }
  if (node.kind === "bool") {
    if (typeof value !== "boolean") failSchema(`${path} must be boolean`);
    return;
  }
  if (node.kind !== "const") return failSchema(`${path} has an unsupported schema kind`);
  if (value !== node.value) failSchema(`${path} violates constant value`);
}

function numericField(value: HostV2Value, key: number, production: string): bigint {
  if (!isMap(value) || !Object.prototype.hasOwnProperty.call(value, key)) {
    return failSchema(`${production} is missing cross-field key ${key}`);
  }
  return integerValue(value[key]!, `${production}/${key}`);
}

function applyCrossFieldRules(production: HostV2TypeName, value: HostV2Value): void {
  for (const rule of HOST_V2_CROSS_FIELD_RULES) {
    if (rule.production !== production) continue;
    if (rule.kind === "uint-positive" && numericField(value, rule.key, production) === 0n) {
      failSchema(`${production} violates ${rule.id}`);
    } else if (rule.kind === "uint-greater" && numericField(value, rule.left, production) <= numericField(value, rule.right, production)) {
      failSchema(`${production} violates ${rule.id}`);
    } else if (rule.kind === "uint-delta-max" && numericField(value, rule.left, production) - numericField(value, rule.right, production) > BigInt(rule.max)) {
      failSchema(`${production} violates ${rule.id}`);
    } else if (rule.kind === "bytes-nonzero") {
      const field = isMap(value) ? value[rule.key] : undefined;
      if (!(field instanceof Uint8Array) || field.every((byte) => byte === 0)) failSchema(`${production} violates ${rule.id}`);
    } else if (rule.kind === "bytes-no-nul") {
      const field = isMap(value) ? value[rule.key] : undefined;
      if (!(field instanceof Uint8Array) || field.includes(0)) failSchema(`${production} violates ${rule.id}`);
    } else if (rule.kind === "bytes-array-sorted-unique") {
      const field = isMap(value) ? value[rule.key] : undefined;
      if (!Array.isArray(field)) failSchema(`${production} violates ${rule.id}`);
      for (let index = 1; index < field.length; index += 1) {
        const previous = field[index - 1];
        const current = field[index];
        const ordered = previous instanceof Uint8Array && current instanceof Uint8Array
          ? compareBytes(previous, current) < 0
          : typeof previous === "string" && typeof current === "string"
            ? compareBytes(utf8Bytes(previous, `${production}/${rule.key}`), utf8Bytes(current, `${production}/${rule.key}`)) < 0
            : false;
        if (!ordered) failSchema(`${production} violates ${rule.id}`);
      }
    }
  }
}

export function encodeHostV2<Name extends HostV2TypeName>(
  production: Name,
  value: HostV2TypeMap[Name],
): Uint8Array {
  const wireValue = value as HostV2Value;
  validateNode(HOST_V2_SCHEMAS[production], wireValue, production);
  applyCrossFieldRules(production, wireValue);
  return encodeHostV2Value(wireValue);
}

export function decodeHostV2<Name extends HostV2TypeName>(
  production: Name,
  bytes: Uint8Array,
): HostV2Dto<Name> {
  const value = decodeCanonicalHostV2Value(bytes);
  validateNode(HOST_V2_SCHEMAS[production], value, production);
  applyCrossFieldRules(production, value);
  return { production, value: value as HostV2TypeMap[Name], canonicalBytes: bytes.slice() };
}
