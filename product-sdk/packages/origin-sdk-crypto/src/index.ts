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

/** Portable Blake2b-256 for browser, mobile, backend, and deterministic runtime vectors. */
const MASK_64 = 0xffff_ffff_ffff_ffffn;
const BLAKE2B_IV = [
  0x6a09e667f3bcc908n, 0xbb67ae8584caa73bn, 0x3c6ef372fe94f82bn, 0xa54ff53a5f1d36f1n,
  0x510e527fade682d1n, 0x9b05688c2b3e6c1fn, 0x1f83d9abfb41bd6bn, 0x5be0cd19137e2179n,
] as const;
const BLAKE2B_SIGMA = [
  [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
  [14, 10, 4, 8, 9, 15, 13, 6, 1, 12, 0, 2, 11, 7, 5, 3],
  [11, 8, 12, 0, 5, 2, 15, 13, 10, 14, 3, 6, 7, 1, 9, 4],
  [7, 9, 3, 1, 13, 12, 11, 14, 2, 6, 5, 10, 4, 0, 15, 8],
  [9, 0, 5, 7, 2, 4, 10, 15, 14, 1, 11, 12, 6, 8, 3, 13],
  [2, 12, 6, 10, 0, 11, 8, 3, 4, 13, 7, 5, 15, 14, 1, 9],
  [12, 5, 1, 15, 14, 13, 4, 10, 0, 7, 6, 3, 9, 2, 8, 11],
  [13, 11, 7, 14, 12, 1, 3, 9, 5, 0, 15, 4, 8, 6, 2, 10],
  [6, 15, 14, 9, 11, 3, 0, 8, 12, 2, 13, 7, 1, 4, 10, 5],
  [10, 2, 8, 4, 7, 6, 1, 5, 15, 11, 9, 14, 3, 12, 13, 0],
  [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
  [14, 10, 4, 8, 9, 15, 13, 6, 1, 12, 0, 2, 11, 7, 5, 3],
] as const;

function rotateRight64(value: bigint, amount: bigint): bigint {
  return ((value >> amount) | (value << (64n - amount))) & MASK_64;
}

function readLittleEndian64(block: Uint8Array, offset: number): bigint {
  let value = 0n;
	for (let index = 0; index < 8; index += 1) value |= BigInt(block[offset + index]!) << BigInt(index * 8);
  return value;
}

export function blake2b256(input: Uint8Array): Uint8Array {
	const state: bigint[] = [...BLAKE2B_IV];
	state[0] = state[0]! ^ 0x0101_0020n;
  let compressedBytes = 0;
  const compress = (block: Uint8Array, last: boolean): void => {
    const words = new Array<bigint>(16);
    for (let index = 0; index < 16; index += 1) words[index] = readLittleEndian64(block, index * 8);
		const work: bigint[] = [...state, ...BLAKE2B_IV];
		work[12] = work[12]! ^ (BigInt(compressedBytes) & MASK_64);
		if (last) work[14] = work[14]! ^ MASK_64;
		const mix = (a: number, b: number, c: number, d: number, x: bigint, y: bigint): void => {
			work[a] = (work[a]! + work[b]! + x) & MASK_64;
			work[d] = rotateRight64(work[d]! ^ work[a]!, 32n);
			work[c] = (work[c]! + work[d]!) & MASK_64;
			work[b] = rotateRight64(work[b]! ^ work[c]!, 24n);
			work[a] = (work[a]! + work[b]! + y) & MASK_64;
			work[d] = rotateRight64(work[d]! ^ work[a]!, 16n);
			work[c] = (work[c]! + work[d]!) & MASK_64;
			work[b] = rotateRight64(work[b]! ^ work[c]!, 63n);
		};
		for (const permutation of BLAKE2B_SIGMA) {
			const word = (position: number): bigint => words[position]!;
			mix(0, 4, 8, 12, word(permutation[0]), word(permutation[1]));
			mix(1, 5, 9, 13, word(permutation[2]), word(permutation[3]));
			mix(2, 6, 10, 14, word(permutation[4]), word(permutation[5]));
			mix(3, 7, 11, 15, word(permutation[6]), word(permutation[7]));
			mix(0, 5, 10, 15, word(permutation[8]), word(permutation[9]));
			mix(1, 6, 11, 12, word(permutation[10]), word(permutation[11]));
			mix(2, 7, 8, 13, word(permutation[12]), word(permutation[13]));
			mix(3, 4, 9, 14, word(permutation[14]), word(permutation[15]));
		}
		for (let index = 0; index < 8; index += 1) state[index] = (state[index]! ^ work[index]! ^ work[index + 8]!) & MASK_64;
  };
  let offset = 0;
  while (offset + 128 < input.byteLength) {
    compressedBytes += 128;
    compress(input.slice(offset, offset + 128), false);
    offset += 128;
  }
  const finalBlock = new Uint8Array(128);
  finalBlock.set(input.slice(offset));
  compressedBytes += input.byteLength - offset;
  compress(finalBlock, true);
  const output = new Uint8Array(32);
  for (let index = 0; index < 4; index += 1) {
		const word = state[index]!;
    for (let byte = 0; byte < 8; byte += 1) output[index * 8 + byte] = Number((word >> BigInt(byte * 8)) & 0xffn);
  }
  return output;
}
