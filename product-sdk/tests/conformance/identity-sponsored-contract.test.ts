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
import { NATIVE_RUNTIME_ROUTE_REGISTRY } from "../../packages/descriptors/src/runtime-route-registry.ts";
import { validateHostRequest, type HostRequest } from "../../packages/host/src/fake-host.ts";
import {
  createTypedNetworkHostRoutes,
  createTypedSponsoredIntentRoutes,
  type TypedPapiClient,
  type TypedRuntimeIdentity,
  type TypedSponsoredIntentTransport,
} from "../../packages/host/src/network-host.ts";
import { identity, type IdentityInfo } from "../../src/identity.ts";
import { sponsoredTransaction, type SignedSponsoredIntent } from "../../src/sponsored-transaction.ts";

const HASH = (byte: string) => `0x${byte.repeat(64)}`;
const PARTICIPANT = "participant-account" as any;
const SPONSOR = "sponsor-account" as any;
const REGISTRAR = "registrar-account" as any;
const identityInfo: IdentityInfo = {
  display: { kind: "raw", value: "Festival Participant" },
  legal: { kind: "none" },
  web: { kind: "none" },
  email: { kind: "none" },
  image: { kind: "blake2_256", hash: HASH("1") as any },
  additional: [{ key: { kind: "raw", value: "tier" }, value: { kind: "raw", value: "enterprise" } }],
};

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
  nonce: "7",
  mortality: { valid_from: "100" as any, valid_until: "164" as any },
  target: { capability: "identity" as const, method: "clear_identity", payload: {} },
  signing_payload_hash: HASH("3") as any,
  intent_id: HASH("2") as any,
};
const signedIntent: SignedSponsoredIntent = {
  envelope,
  participant_signature: { scheme: "sr25519", value: `0x${"33".repeat(64)}` },
};

test("identity factories expose only the nine audit-approved closed routes", () => {
  const requests = [
    identity.identityStatus(context("identity:identity_status"), PARTICIPANT),
    identity.personhoodStatus(context("identity:personhood_status"), PARTICIPANT),
    identity.attestationAllowance(context("identity:attestation_allowance"), PARTICIPANT),
    identity.setIdentity(context("identity:set_identity"), identityInfo),
    identity.clearIdentity(context("identity:clear_identity")),
    identity.requestJudgement(context("identity:request_judgement"), REGISTRAR),
    identity.cancelJudgementRequest(context("identity:cancel_judgement_request"), REGISTRAR),
    identity.provideJudgement(
      context("identity:provide_judgement"),
      PARTICIPANT,
      "known_good",
      HASH("4") as any,
    ),
    identity.attestLitePerson(context("identity:attest_lite_person"), {
      candidate: PARTICIPANT,
      candidate_signature: { scheme: "sr25519", bytes: `0x${"11".repeat(64)}` },
      ring_vrf_key: HASH("2") as any,
      proof_of_ownership: `0x${"22".repeat(64)}` as any,
    }),
  ];
  assert.deepEqual(requests.map(({ capability, method }) => `${capability}:${method}`), [
    "identity:identity_status",
    "identity:personhood_status",
    "identity:attestation_allowance",
    "identity:set_identity",
    "identity:clear_identity",
    "identity:request_judgement",
    "identity:cancel_judgement_request",
    "identity:provide_judgement",
    "identity:attest_lite_person",
  ]);
  requests.forEach(validateHostRequest);
  assert.deepEqual(requests.map(({ finality }) => finality), [
    "finalized", "finalized", "finalized",
    "submit-and-finalize", "submit-and-finalize", "submit-and-finalize",
    "submit-and-finalize", "submit-and-finalize", "submit-and-finalize",
  ]);
  for (const route of requests.map(({ capability, method }) => `${capability}:${method}`)) {
    assert.equal(typeof (NATIVE_RUNTIME_ROUTE_REGISTRY as any)[route], "function", route);
  }
  assert.equal(typeof NATIVE_RUNTIME_ROUTE_REGISTRY["transaction:prepare_sponsored_intent"], "function");
  assert.equal(typeof NATIVE_RUNTIME_ROUTE_REGISTRY["transaction:submit_sponsored_intent"], "function");
});

test("identity and personhood payloads fail closed on unbounded or invented fields", () => {
  assert.equal(code(() => assertMethodPayload("identity", "set_identity", {
    info: { ...identityInfo, display: { kind: "raw", value: "x".repeat(33) } },
  } as any)), "invalid_input");
  assert.equal(code(() => assertMethodPayload("identity", "attest_lite_person", {
    candidate: PARTICIPANT,
    candidate_signature: { scheme: "sr25519", bytes: `0x${"11".repeat(64)}` },
    ring_vrf_key: HASH("2"),
    proof_of_ownership: `0x${"22".repeat(64)}`,
    consumer_registration: null,
  } as any)), "invalid_input");
});

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
  for (const method of ["prepare_sponsored_intent", "submit_sponsored_intent"]) {
    const canonical = canonicalMethodPayload("transaction", method);
    assertMethodPayload("transaction", method, canonical);
    const schema = methodPayloadSchema("transaction", method) as any;
    assert.equal(schema.additionalProperties, false);
    assert.equal(JSON.stringify(schema).match(/raw.?scale|pallet_index|call_index|contract_abi/i), null);
  }
});

class Client implements TypedPapiClient {
  identity: TypedRuntimeIdentity = {
    genesis_hash: ORBIS_NETWORK_BINDING.genesis_hash,
    spec_version: ORBIS_NETWORK_BINDING.spec_version,
    transaction_version: ORBIS_NETWORK_BINDING.transaction_version,
    metadata_hash: ORBIS_NETWORK_BINDING.metadata_hash,
  };
  async getFinalizedBlock() { return { hash: HASH("a") }; }
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
