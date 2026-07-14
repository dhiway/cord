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

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import test from "node:test";
import {
  SUPPORTED_DELEGATED_SIGNATURE_SCHEMES,
  attestationEventOutcome,
  attestationEventSubscription,
  delegatedIssueSigningPayload,
  delegatedRevokeSigningPayload,
  signingPayloadHex,
  type AttestationEvent,
  type AttestationOutcome,
  type DelegatedIssueIntent,
  type DelegatedRevokeIntent,
} from "../../src/attestation.ts";
import type { BlockHash } from "../../src/types.ts";

const vectors = JSON.parse(
  readFileSync(resolve(import.meta.dirname, "../../../docs/sdk/vectors/attestation-v1.json"), "utf8"),
) as {
  signing: Array<{
    scheme: string;
    kind: "issue" | "revoke";
    intent?: unknown;
    same_as?: string;
    payload: string;
  }>;
  events: Array<{ runtime_event: AttestationEvent; semantic_outcome: AttestationOutcome }>;
  source_symbols: string[];
};

const EIP712_SOURCE_SYMBOLS = [
  "EIP712Verifier.ATTEST_TYPEHASH",
  "EIP712Verifier.EIP712Verifier__DeadlineExpired",
  "EIP712Verifier.EIP712Verifier__InvalidNonce",
  "EIP712Verifier.EIP712Verifier__InvalidSignature",
  "EIP712Verifier.NonceIncreased",
  "EIP712Verifier.REVOKE_TYPEHASH",
  "EIP712Verifier._nonces",
  "EIP712Verifier._time",
  "EIP712Verifier._verifyAttest",
  "EIP712Verifier._verifyRevoke",
  "EIP712Verifier.constructor",
  "EIP712Verifier.getAttestTypeHash",
  "EIP712Verifier.getDomainSeparator",
  "EIP712Verifier.getName",
  "EIP712Verifier.getNonce",
  "EIP712Verifier.getRevokeTypeHash",
  "EIP712Verifier.increaseNonce",
  "EIP712Verifier.roles-storage-economic-signature-lifecycle",
] as const;

test("Rust and TypeScript share canonical delegated signing payloads for every scheme", () => {
  assert.deepEqual(
    vectors.source_symbols.filter((symbol) => symbol.startsWith("EIP712Verifier.")).sort(),
    [...EIP712_SOURCE_SYMBOLS].sort(),
  );
  assert.deepEqual(
    [...new Set(vectors.signing.map(({ scheme }) => scheme))].sort(),
    [...SUPPORTED_DELEGATED_SIGNATURE_SCHEMES].sort(),
  );
  const canonical = new Map<string, unknown>();
  for (const vector of vectors.signing) {
    if (vector.intent) canonical.set(vector.kind, vector.intent);
    const intent = vector.intent ?? canonical.get(vector.kind);
    assert.ok(intent, `missing canonical ${vector.kind} intent`);
    const payload =
      vector.kind === "issue"
        ? delegatedIssueSigningPayload(intent as DelegatedIssueIntent)
        : delegatedRevokeSigningPayload(intent as DelegatedRevokeIntent);
    assert.equal(signingPayloadHex(payload), vector.payload, `${vector.scheme}:${vector.kind}`);
  }
});

test("native attestation events map to stable finalized semantic outcomes", () => {
  for (const vector of vectors.events) {
    assert.deepEqual(attestationEventOutcome(vector.runtime_event), vector.semantic_outcome);
  }
  assert.deepEqual(
    attestationEventSubscription(`0x${"11".repeat(32)}` as BlockHash, [
      "attestation_issued",
      "delegated_intent_consumed",
      "delegated_revocation_consumed",
      "attestation_revoked",
    ]),
    {
      finality: "finalized",
      from_finalized_block: `0x${"11".repeat(32)}`,
      kinds: [
        "attestation_issued",
        "delegated_intent_consumed",
        "delegated_revocation_consumed",
        "attestation_revoked",
      ],
    },
  );
});
