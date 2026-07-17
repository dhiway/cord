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
import test from "node:test";
import {
  assertMethodPayload,
  assertNoContractSurface,
  canonicalMethodPayload,
  methodPayloadSchema,
  ProductSdkError,
  type JsonObject,
} from "../../packages/core/src/contract.ts";
import {
  ORBIS_CANDIDATE_NETWORK_BINDING,
  ORBIS_NETWORK_BINDING,
} from "../../packages/descriptors/generated/orbis-network-binding.ts";
import { validateHostRequest, type HostRequest } from "../../packages/host/src/fake-host.ts";
import {
  createTypedNetworkHostRoutes,
  createTypedSponsoredIntentRoutes,
  type TypedPapiClient,
  type TypedRuntimeIdentity,
  type TypedSponsoredIntentTransport,
} from "../../packages/host/src/network-host.ts";
import {
  sponsoredNonce,
  sponsoredTransaction,
  type SignedSponsoredIntent,
  type SponsoredNativeTarget,
} from "../../src/sponsored-transaction.ts";

const HASH = (byte: string) => `0x${byte.repeat(64)}`;
const PARTICIPANT = "participant-account" as any;
const SPONSOR = "sponsor-account" as any;
type AssertFalse<Value extends false> = Value;
type _SponsoredMethodIsClosed = AssertFalse<string extends SponsoredNativeTarget["method"] ? true : false>;
type _InventedTargetIsRejected = AssertFalse<{
  capability: "identity";
  method: "invented";
  payload: {};
} extends SponsoredNativeTarget ? true : false>;
let sequence = 0;
function context(scope: string) {
  sequence++;
  const tag = String(sequence).padStart(4, "0");
  return {
    request_id: `identity-route-request-${tag}`,
    application_id: "festival",
    network: { ...ORBIS_CANDIDATE_NETWORK_BINDING },
    consent: { scopes: [scope], expires_at: 2_000, nonce: `identity-route-consent-${tag}` },
  };
}

function code(operation: () => unknown): string {
  try { operation(); return "success"; }
  catch (error) { assert.ok(error instanceof ProductSdkError); return error.code; }
}

const envelope = {
  version: 1 as const,
  signing_domain: "orbis/meta-intent/v7" as const,
  genesis_hash: ORBIS_NETWORK_BINDING.genesis_hash,
  spec_version: ORBIS_NETWORK_BINDING.spec_version,
  transaction_version: ORBIS_NETWORK_BINDING.transaction_version,
  metadata_hash: ORBIS_NETWORK_BINDING.metadata_hash,
  participant: PARTICIPANT,
  nonce: sponsoredNonce(7),
  mortality: { valid_from: "100" as any, valid_until: "164" as any },
  target: { capability: "attestation", method: "revoke", payload: { attestation: HASH("4") } } as const,
  signing_payload_hash: HASH("3") as any,
  intent_id: HASH("2") as any,
};
const signedIntent: SignedSponsoredIntent = {
  envelope,
  participant_signature: { scheme: "sr25519", value: `0x${"33".repeat(64)}` },
};

test("sponsored factories carry typed targets and participant signatures without raw SCALE", () => {
  assert.doesNotThrow(() => assertNoContractSurface({ capability: "identity" }));
  assert.throws(
    () => assertNoContractSurface({ abi_payload: "forbidden" }),
    (error: ProductSdkError) => error.code === "unsupported_surface",
  );
  const prepare = sponsoredTransaction.prepareSponsoredIntent(
    context("transaction:prepare_sponsored_intent"),
    {
      participant: PARTICIPANT,
      nonce: "7",
      mortality: envelope.mortality,
      target: envelope.target,
    },
  );
  const submit = sponsoredTransaction.submitSponsoredIntent(
    context("transaction:submit_sponsored_intent"),
    signedIntent,
  );
  validateHostRequest(prepare);
  validateHostRequest(submit);
  assert.deepEqual([prepare.finality, submit.finality], ["finalized", "submit-and-finalize"]);
  assert.equal(JSON.stringify([prepare, submit]).match(/raw.?scale|pallet_index|call_index|contract_abi/i), null);
  assert.equal(code(() => assertMethodPayload("transaction", "prepare_sponsored_intent", {
    ...prepare.payload,
    target: { capability: "transaction", method: "submit_sponsored_intent", payload: {} },
  })), "invalid_input");
  assert.equal(code(() => assertMethodPayload("transaction", "prepare_sponsored_intent", {
    ...prepare.payload,
    mortality: { valid_from: "100", valid_until: "100" },
  })), "invalid_input");
  assert.equal(code(() => assertMethodPayload("transaction", "prepare_sponsored_intent", {
    ...prepare.payload,
    nonce: "4294967296",
  })), "invalid_input");
  assert.equal(code(() => assertMethodPayload("transaction", "prepare_sponsored_intent", {
    ...prepare.payload,
    nonce: "07",
  })), "invalid_input");
  assert.equal(code(() => assertMethodPayload("transaction", "prepare_sponsored_intent", {
    ...prepare.payload,
    mortality: { valid_from: "100", valid_until: "165" },
  })), "invalid_input");
  assert.equal(code(() => assertMethodPayload("transaction", "prepare_sponsored_intent", {
    ...prepare.payload,
    mortality: { valid_from: "101", valid_until: "8293" },
  })), "invalid_input");
  assert.equal(code(() => assertMethodPayload("transaction", "prepare_sponsored_intent", {
    ...prepare.payload,
    mortality: { valid_from: "100", valid_until: "102" },
  })), "invalid_input");
  assert.equal(code(() => assertMethodPayload("transaction", "prepare_sponsored_intent", {
    ...prepare.payload,
    mortality: { valid_from: "100", valid_until: "131172" },
  })), "invalid_input");
  assert.doesNotThrow(() => assertMethodPayload("transaction", "prepare_sponsored_intent", {
    ...prepare.payload,
    mortality: { valid_from: "112", valid_until: "65648" },
  }));

  const signedPayload = submit.payload.signed_intent as any;
  for (const participant_signature of [
    { scheme: "sr25519", value: `0x${"33".repeat(63)}` },
    { scheme: "ed25519", value: `0x${"AA".repeat(64)}` },
    { scheme: "ecdsa", value: `0x${"33".repeat(64)}` },
    { scheme: "sr25519", value: `0x${"33".repeat(65)}` },
  ]) {
    assert.equal(code(() => assertMethodPayload("transaction", "submit_sponsored_intent", {
      signed_intent: { ...signedPayload, participant_signature },
    })), "invalid_input");
  }
  for (const participant_signature of [
    { scheme: "sr25519", value: `0x${"33".repeat(64)}` },
    { scheme: "ed25519", value: `0x${"44".repeat(64)}` },
    { scheme: "ecdsa", value: `0x${"55".repeat(65)}` },
  ]) {
    assert.doesNotThrow(() => assertMethodPayload("transaction", "submit_sponsored_intent", {
      signed_intent: { ...signedPayload, participant_signature },
    }));
  }
  for (const method of ["prepare_sponsored_intent", "submit_sponsored_intent"]) {
    const canonical = canonicalMethodPayload("transaction", method);
    assertMethodPayload("transaction", method, canonical);
    const schema = methodPayloadSchema("transaction", method) as any;
    assert.equal(schema.additionalProperties, false);
    assert.equal(JSON.stringify(schema).match(/raw.?scale|pallet_index|call_index|contract_abi/i), null);
  }
});

test("sponsored nonce constructor accepts only canonical u32 decimal values", () => {
  assert.equal(sponsoredNonce("0"), "0");
  assert.equal(sponsoredNonce("4294967295"), "4294967295");
  for (const invalid of ["-1", "01", "4294967296", "1.0", Number.NaN, Number.POSITIVE_INFINITY]) {
    assert.throws(() => sponsoredNonce(invalid), TypeError);
  }
});

class Client implements TypedPapiClient {
  identity: TypedRuntimeIdentity = {
    genesis_hash: ORBIS_NETWORK_BINDING.genesis_hash,
    spec_version: ORBIS_NETWORK_BINDING.spec_version,
    transaction_version: ORBIS_NETWORK_BINDING.transaction_version,
    metadata_hash: ORBIS_NETWORK_BINDING.metadata_hash,
  };
  async getFinalizedBlock() { return { hash: HASH("a"), number: "100" }; }
  async getRuntimeIdentityAt() { return this.identity; }
}

function transaction(events: any[] | undefined) {
  return {
    async *signSubmitAndWatch() {
      yield { type: "finalized" as const, blockHash: HASH("b"), extrinsicHash: HASH("c"), events };
    },
  };
}

async function asyncCode(operation: Promise<unknown>): Promise<string> {
  try { await operation; return "success"; }
  catch (error) { assert.ok(error instanceof ProductSdkError); return error.code; }
}

test("typed sponsored boundary requires distinct outer sponsor and MetaTx::Dispatched Ok", async () => {
  const client = new Client();
  let finalizedEvents: any[] | undefined = [
    { pallet: "MetaTx", event: "Dispatched", fields: { result: "Ok" } },
  ];
  const transport: TypedSponsoredIntentTransport<Client, { accountId: string }> = {
    async prepare(payload) {
      return {
        version: 1,
        signing_domain: "orbis/meta-intent/v7",
        genesis_hash: ORBIS_NETWORK_BINDING.genesis_hash,
        spec_version: ORBIS_NETWORK_BINDING.spec_version,
        transaction_version: ORBIS_NETWORK_BINDING.transaction_version,
        metadata_hash: ORBIS_NETWORK_BINDING.metadata_hash,
        ...payload,
        signing_payload_hash: envelope.signing_payload_hash,
        intent_id: envelope.intent_id,
      } as any;
    },
    submit() { return transaction(finalizedEvents); },
  };
  const dependencies = createTypedNetworkHostRoutes({
    client,
    signer: { accountId: SPONSOR },
    binding: ORBIS_NETWORK_BINDING,
    routes: createTypedSponsoredIntentRoutes(transport),
  });
  const submit = sponsoredTransaction.submitSponsoredIntent(
    context("transaction:submit_sponsored_intent"),
    signedIntent,
  ) as unknown as HostRequest;
  const prepare = sponsoredTransaction.prepareSponsoredIntent(
    context("transaction:prepare_sponsored_intent"),
    {
      participant: PARTICIPANT,
      nonce: envelope.nonce,
      mortality: envelope.mortality,
      target: envelope.target,
    },
  ) as unknown as HostRequest;
  const prepared = await dependencies.finalizedRead!(
    { request: prepare, signature: "host-authorization" },
    new AbortController().signal,
  );
  assert.equal((prepared.response as any).intent_id, envelope.intent_id);
  const result = await dependencies.submitAndFinalize!({ request: submit, signature: "host-authorization" }, new AbortController().signal);
  assert.deepEqual(result.response, {
    version: 1,
    intent_id: envelope.intent_id,
    participant: PARTICIPANT,
    sponsor: SPONSOR,
    dispatched: true,
    meta_tx_event: "Dispatched",
    inner_result: "Ok",
  });

  finalizedEvents = undefined;
  assert.equal(await asyncCode(dependencies.submitAndFinalize!(
    { request: submit, signature: "host-authorization" },
    new AbortController().signal,
  )), "runtime_rejected");

  finalizedEvents = [
    { pallet: "MetaTx", event: "Dispatched", fields: { result: "Ok" } },
  ];
  const alternateSponsor = createTypedNetworkHostRoutes({
    client,
    signer: { accountId: "other-sponsor" },
    binding: ORBIS_NETWORK_BINDING,
    routes: createTypedSponsoredIntentRoutes(transport),
  });
  assert.equal(await asyncCode(alternateSponsor.submitAndFinalize!(
    { request: submit, signature: "host-authorization" },
    new AbortController().signal,
  )), "success");

  const participantAsSponsor = createTypedNetworkHostRoutes({
    client,
    signer: { accountId: PARTICIPANT },
    binding: ORBIS_NETWORK_BINDING,
    routes: createTypedSponsoredIntentRoutes(transport),
  });
  assert.equal(await asyncCode(participantAsSponsor.submitAndFinalize!(
    { request: submit, signature: "host-authorization" },
    new AbortController().signal,
  )), "invalid_input");
});

test("typed sponsored preparation is anchored to the exact finalized block number", async () => {
  const client = new Client();
  let prepareCalls = 0;
  const transport: TypedSponsoredIntentTransport<Client, { accountId: string }> = {
    async prepare(payload) {
      prepareCalls++;
      return {
        version: 1,
        signing_domain: "orbis/meta-intent/v7",
        genesis_hash: ORBIS_NETWORK_BINDING.genesis_hash,
        spec_version: ORBIS_NETWORK_BINDING.spec_version,
        transaction_version: ORBIS_NETWORK_BINDING.transaction_version,
        metadata_hash: ORBIS_NETWORK_BINDING.metadata_hash,
        ...payload,
        signing_payload_hash: envelope.signing_payload_hash,
        intent_id: envelope.intent_id,
      } as any;
    },
    submit() { return transaction([]); },
  };
  const dependencies = createTypedNetworkHostRoutes({
    client,
    signer: { accountId: SPONSOR },
    binding: ORBIS_NETWORK_BINDING,
    routes: createTypedSponsoredIntentRoutes(transport),
  });
  const stale = sponsoredTransaction.prepareSponsoredIntent(
    context("transaction:prepare_sponsored_intent"),
    {
      participant: PARTICIPANT,
      nonce: sponsoredNonce(7),
      mortality: { valid_from: "99" as any, valid_until: "163" as any },
      target: envelope.target,
    },
  ) as unknown as HostRequest;
  assert.equal(await asyncCode(dependencies.finalizedRead!(
    { request: stale, signature: "host-authorization" },
    new AbortController().signal,
  )), "invalid_input");
  assert.equal(prepareCalls, 0);
});
