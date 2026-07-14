import assert from "node:assert/strict";
import {
  createHash,
  createPrivateKey,
  createPublicKey,
  sign as signDetached,
  verify as verifyDetached,
} from "node:crypto";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";
import { ProductSdkError, type JsonObject, type JsonValue } from "../../packages/core/src/contract.ts";
import {
  ORBIS_CANDIDATE_NETWORK_BINDING,
  ORBIS_NETWORK_BINDING,
} from "../../packages/descriptors/generated/orbis-network-binding.ts";
import { NATIVE_RUNTIME_ROUTE_REGISTRY } from "../../packages/descriptors/src/runtime-route-registry.ts";
import { FakeHost, type HostRequest } from "../../packages/host/src/fake-host.ts";
import {
  createTypedSponsoredIntentRoutes,
  createTypedNetworkHostRoutes,
  type TypedChainSigner,
  type TypedNetworkRoutes,
  type TypedPapiClient,
  type TypedRuntimeIdentity,
  type TypedTransactionStatus,
} from "../../packages/host/src/network-host.ts";
import { normalizedLabel, registrationSalt, textKey, textValue } from "../../src/names.ts";

const APP_ID = "festival-p6-reference";
const FINALIZED_HASH = `0x${"a1".repeat(32)}`;
const FINALIZED_NUMBER = "100";
const SUBMISSION_HASH = `0x${"b2".repeat(32)}`;
const PARTICIPANT_PRIVATE_KEY = createPrivateKey({
  key: Buffer.concat([
    Buffer.from("302e020100300506032b657004220420", "hex"),
    Buffer.alloc(32, 0x19),
  ]),
  format: "der",
  type: "pkcs8",
});
const PARTICIPANT_PUBLIC_KEY = createPublicKey(PARTICIPANT_PRIVATE_KEY);
const PARTICIPANT_PUBLIC_DER = PARTICIPANT_PUBLIC_KEY.export({ format: "der", type: "spki" });
const PARTICIPANT_ACCOUNT = `0x${PARTICIPANT_PUBLIC_DER.subarray(-32).toString("hex")}`;

function canonicalJson(value: JsonValue): string {
  if (value === null || typeof value !== "object") return JSON.stringify(value);
  if (Array.isArray(value)) return `[${value.map(canonicalJson).join(",")}]`;
  return `{${Object.keys(value).sort().map((key) =>
    `${JSON.stringify(key)}:${canonicalJson(value[key] as JsonValue)}`).join(",")}}`;
}

function sponsoredEnvelopeHash(envelope: JsonObject): string {
  const committed = {
    version: envelope.version,
    signing_domain: envelope.signing_domain,
    genesis_hash: envelope.genesis_hash,
    spec_version: envelope.spec_version,
    transaction_version: envelope.transaction_version,
    metadata_hash: envelope.metadata_hash,
    participant: envelope.participant,
    nonce: envelope.nonce,
    mortality: envelope.mortality,
    target: envelope.target,
  } as JsonObject;
  return `0x${createHash("sha256").update(canonicalJson(committed)).digest("hex")}`;
}

function sponsoredIntentId(signingPayloadHash: string): string {
  return `0x${createHash("sha256").update(`orbis/meta-intent/v7:${signingPayloadHash}`).digest("hex")}`;
}
const CREDENTIAL_ID = `0x${"11".repeat(32)}`;
const SCHEMA_ID = `0x${"13".repeat(32)}`;
const NAME_ID = `0x${"14".repeat(32)}`;
const REGISTRATION_COMMITMENT = `0x${"15".repeat(32)}`;
const SUBJECT = `0x${"16".repeat(32)}`;
const PAYLOAD = `0x${"17".repeat(32)}`;
const STATUS = `0x${"18".repeat(32)}`;

type Terminal = {
  code: string;
  retryable: boolean;
  finalized_hash?: string;
  extrinsic_hash?: string;
  response?: JsonValue;
};

interface JourneyState {
  credentialLive: boolean;
  sponsorCredits: number;
  selfPauseNext: boolean;
  online: boolean;
  events: Array<{ finalized_block_hash: string; event_index: number; event: JsonObject }>;
  participantAuthorizations: string[];
  selfChainSigners: string[];
  sponsorChainSigners: string[];
  selfRouteSelections: string[];
  sponsorRouteSelections: string[];
  sponsoredIntentSequence: number;
  consumedIntents: Set<string>;
}

class JourneyClient implements TypedPapiClient {
  readonly runtimeIdentityCalls: string[] = [];
  private readonly state: JourneyState;
  identity: TypedRuntimeIdentity = {
    genesis_hash: ORBIS_NETWORK_BINDING.genesis_hash,
    spec_version: ORBIS_NETWORK_BINDING.spec_version,
    transaction_version: ORBIS_NETWORK_BINDING.transaction_version,
    metadata_hash: ORBIS_NETWORK_BINDING.metadata_hash,
  };

  constructor(state: JourneyState) { this.state = state; }

  async getFinalizedBlock(): Promise<{ hash: string; number: string }> {
    if (!this.state.online) throw { code: "network", message: "festival device is offline", retryable: true };
    return { hash: FINALIZED_HASH, number: FINALIZED_NUMBER };
  }

  async getRuntimeIdentityAt(hash: string): Promise<TypedRuntimeIdentity> {
    if (!this.state.online) throw { code: "network", message: "festival device is offline", retryable: true };
    this.runtimeIdentityCalls.push(hash);
    return this.identity;
  }
}

function finalizedTransaction(
  operation: () => void,
  signerAudit: string[],
  routeAudit: string[],
  route: string,
  pause: () => boolean,
) {
  return {
    async *signSubmitAndWatch(
      signer: TypedChainSigner,
      options: { readonly signal: AbortSignal },
    ): AsyncIterable<TypedTransactionStatus> {
      signerAudit.push(signer.accountId);
      routeAudit.push(route);
      yield { type: "broadcasted" };
      if (pause()) {
        await new Promise<never>((_, reject) => options.signal.addEventListener(
          "abort", () => reject(new ProductSdkError("cancelled", "festival operation cancelled")),
          { once: true },
        ));
      }
      operation();
      yield { type: "finalized", blockHash: SUBMISSION_HASH, extrinsicHash: `0x${"c3".repeat(32)}` };
    },
  };
}

function selfRoutes(state: JourneyState): TypedNetworkRoutes<JourneyClient, TypedChainSigner> {
  return {
    "identity:personhood_status": {
      finality: "finalized",
      async query(_payload, context): Promise<JsonValue> {
        state.selfRouteSelections.push("identity:personhood_status");
        state.participantAuthorizations.push(context.authorizationSignature);
        return { version: 1, full_personal_id: "42", full_recognized: true, lite_recognized: true };
      },
    },
    "names:root_name_by_normalized_label": {
      finality: "finalized",
      async query(payload, context): Promise<JsonValue> {
        state.selfRouteSelections.push("names:root_name_by_normalized_label");
        state.participantAuthorizations.push(context.authorizationSignature);
        return { version: 1, value: { name: NAME_ID, label: payload.label, owner: "participant" } };
      },
    },
    "names:resolve_attestation": {
      finality: "finalized",
      async query(_payload, context): Promise<JsonValue> {
        state.selfRouteSelections.push("names:resolve_attestation");
        state.participantAuthorizations.push(context.authorizationSignature);
        return { version: 1, value: CREDENTIAL_ID };
      },
    },
    "names:commit": {
      finality: "submit-and-finalize",
      transaction(_payload, context) {
        state.participantAuthorizations.push(context.authorizationSignature);
        return finalizedTransaction(
          () => undefined,
          state.selfChainSigners,
          state.selfRouteSelections,
          "names:commit",
          () => {
            const paused = state.selfPauseNext;
            state.selfPauseNext = false;
            return paused;
          },
        );
      },
    },
    "names:register": {
      finality: "submit-and-finalize",
      transaction(payload, context) {
        state.participantAuthorizations.push(context.authorizationSignature);
        return finalizedTransaction(
          () => state.events.push({
            finalized_block_hash: SUBMISSION_HASH,
            event_index: state.events.length,
            event: {
              event: "name_registered",
              data: { name: NAME_ID, label: payload.label, owner: "participant" },
            },
          }),
          state.selfChainSigners,
          state.selfRouteSelections,
          "names:register",
          () => false,
        );
      },
    },
  };
}

function sponsorRoutes(state: JourneyState): TypedNetworkRoutes<JourneyClient, TypedChainSigner> {
  const sponsored = createTypedSponsoredIntentRoutes<JourneyClient, TypedChainSigner>({
    async prepare(payload, context): Promise<JsonValue> {
      state.sponsorRouteSelections.push("transaction:prepare_sponsored_intent");
      state.participantAuthorizations.push(context.authorizationSignature);
      state.sponsoredIntentSequence++;
      const envelope = {
        version: 1,
        signing_domain: "orbis/meta-intent/v7",
        genesis_hash: ORBIS_NETWORK_BINDING.genesis_hash,
        spec_version: ORBIS_NETWORK_BINDING.spec_version,
        transaction_version: ORBIS_NETWORK_BINDING.transaction_version,
        metadata_hash: ORBIS_NETWORK_BINDING.metadata_hash,
        participant: payload.participant,
        nonce: payload.nonce,
        mortality: payload.mortality,
        target: payload.target,
      } as JsonObject;
      const signingPayloadHash = sponsoredEnvelopeHash(envelope);
      return {
        ...envelope,
        signing_payload_hash: signingPayloadHash,
        intent_id: sponsoredIntentId(signingPayloadHash),
      };
    },
    submit(payload, context) {
      state.participantAuthorizations.push(context.authorizationSignature);
      return {
        async *signSubmitAndWatch(signer: TypedChainSigner): AsyncIterable<TypedTransactionStatus> {
          state.sponsorChainSigners.push(signer.accountId);
          state.sponsorRouteSelections.push("transaction:submit_sponsored_intent");
          const signed = payload.signed_intent as JsonObject;
          const envelope = signed.envelope as JsonObject;
          const signature = signed.participant_signature as JsonObject;
          const signingPayloadHash = sponsoredEnvelopeHash(envelope);
          const intentId = sponsoredIntentId(signingPayloadHash);
          const signatureValue = String(signature.value);
          const signatureBytes = /^0x[0-9a-f]{128}$/.test(signatureValue)
            ? Buffer.from(signatureValue.slice(2), "hex")
            : Buffer.alloc(0);
          if (
            signature.scheme !== "ed25519"
            || envelope.participant !== PARTICIPANT_ACCOUNT
            || envelope.signing_payload_hash !== signingPayloadHash
            || envelope.intent_id !== intentId
            || !verifyDetached(
              null,
              Buffer.from(signingPayloadHash.slice(2), "hex"),
              PARTICIPANT_PUBLIC_KEY,
              signatureBytes,
            )
          ) {
            yield {
              type: "rejected",
              error: { code: "proof_invalid", message: "participant intent signature mismatch", retryable: false },
            };
            return;
          }
          if (state.consumedIntents.has(intentId)) {
            yield {
              type: "rejected",
              error: { code: "replay", message: "sponsored intent was already consumed", retryable: false },
            };
            return;
          }
          if (!state.credentialLive) {
            yield {
              type: "rejected",
              error: { code: "not_authorized", message: "credential evidence is not live", retryable: false },
            };
            return;
          }
          if (state.sponsorCredits < 1) {
            yield {
              type: "rejected",
              error: { code: "capacity_exceeded", message: "sponsor budget exhausted", retryable: false },
            };
            return;
          }
          state.sponsorCredits--;
          state.consumedIntents.add(intentId);
          yield { type: "broadcasted" };
          state.events.push({
            finalized_block_hash: SUBMISSION_HASH,
            event_index: state.events.length,
            event: {
              event: "sponsored_check_in",
              data: {
                intent: envelope.intent_id,
                participant: envelope.participant,
                sponsor: signer.accountId,
              },
            },
          });
          yield {
            type: "finalized",
            blockHash: SUBMISSION_HASH,
            extrinsicHash: `0x${"d4".repeat(32)}`,
            events: [{ pallet: "MetaTx", event: "Dispatched", fields: { result: "Ok" } }],
          };
        },
      };
    },
  });
  return {
    ...sponsored,
    "attestation:attestation_live_status": {
      finality: "finalized",
      async query(_payload, context): Promise<JsonValue> {
        state.sponsorRouteSelections.push("attestation:attestation_live_status");
        state.participantAuthorizations.push(context.authorizationSignature);
        return {
          version: 1,
          exists: true,
          live: state.credentialLive,
          evaluated_at: "100",
          expiry: null,
          revoked_at: state.credentialLive ? null : "99",
        };
      },
    },
    "attestation:revoke": {
      finality: "submit-and-finalize",
      transaction(payload, context) {
        state.participantAuthorizations.push(context.authorizationSignature);
        return finalizedTransaction(
          () => {
            state.credentialLive = false;
            state.events.push({
              finalized_block_hash: SUBMISSION_HASH,
              event_index: state.events.length,
              event: {
                event: "attestation_revoked",
                data: { attestation: payload.attestation, by: "sponsor" },
              },
            });
          },
          state.sponsorChainSigners,
          state.sponsorRouteSelections,
          "attestation:revoke",
          () => false,
        );
      },
    },
  };
}

function terminalError(error: unknown): Terminal {
  assert.ok(error instanceof ProductSdkError, `expected ProductSdkError, received ${String(error)}`);
  return { code: error.code, retryable: error.retryable };
}

async function execute(host: FakeHost, request: HostRequest): Promise<Terminal> {
  try {
    host.issueConsent(request.application_id, request.consent);
    const result = await host.execute(request);
    return {
      code: "success",
      retryable: false,
      finalized_hash: result.finalizedHash,
      ...(result.extrinsicHash ? { extrinsic_hash: result.extrinsicHash } : {}),
      ...(result.response !== undefined ? { response: result.response } : {}),
    };
  } catch (error) {
    return terminalError(error);
  }
}

function readManifest(): any {
  return JSON.parse(readFileSync(resolve(import.meta.dirname, "p6-journey.manifest.json"), "utf8"));
}

export async function runFestivalJourney(): Promise<JsonObject> {
  const manifest = readManifest();
  const state: JourneyState = {
    credentialLive: true,
    sponsorCredits: 1,
    selfPauseNext: false,
    online: true,
    events: [],
    participantAuthorizations: [],
    selfChainSigners: [],
    sponsorChainSigners: [],
    selfRouteSelections: [],
    sponsorRouteSelections: [],
    sponsoredIntentSequence: 0,
    consumedIntents: new Set(),
  };
  const selfClient = new JourneyClient(state);
  const sponsorClient = new JourneyClient(state);
  const selfSigner = { accountId: PARTICIPANT_ACCOUNT } as const;
  const sponsorSigner = { accountId: "festival-sponsor-account" } as const;
  const authorizationSigner = async (request: HostRequest) => `participant-authorization:${request.request_id}`;
  const terminal: string[] = [];
  const sponsoredRoutes = sponsorRoutes(state);
  const selfHost = new FakeHost({
    signer: authorizationSigner,
    ...createTypedNetworkHostRoutes({
      client: selfClient,
      signer: selfSigner,
      binding: ORBIS_NETWORK_BINDING,
      routes: { ...selfRoutes(state), ...sponsoredRoutes },
    }),
    onTerminal: (requestId, outcome) => terminal.push(`${requestId}:${outcome}`),
  });
  const sponsorHost = new FakeHost({
    signer: authorizationSigner,
    ...createTypedNetworkHostRoutes({
      client: sponsorClient,
      signer: sponsorSigner,
      binding: ORBIS_NETWORK_BINDING,
      routes: sponsoredRoutes,
    }),
    onTerminal: (requestId, outcome) => terminal.push(`${requestId}:${outcome}`),
  });
  selfHost.grant(APP_ID, manifest.permissions.self_host);
  sponsorHost.grant(APP_ID, manifest.permissions.sponsor_host);

  let sequence = 0;
  const build = (scope: keyof typeof NATIVE_RUNTIME_ROUTE_REGISTRY, args: readonly unknown[]): HostRequest => {
    sequence++;
    const tag = String(sequence).padStart(4, "0");
    const context = {
      request_id: `festival-p6-request-${tag}`,
      application_id: APP_ID,
      network: { ...ORBIS_CANDIDATE_NETWORK_BINDING },
      consent: {
        scopes: [scope],
        expires_at: 2_000,
        nonce: `festival-p6-consent-${tag}`,
      },
    };
    const route = NATIVE_RUNTIME_ROUTE_REGISTRY[scope] as (context: typeof context, ...values: any[]) => HostRequest;
    return route(context, ...args);
  };

  const results: Record<string, Terminal> = {};

  const denied = build("names:set_text", [NAME_ID, textKey("festival"), textValue("denied")]);
  results.permission_denial = await execute(selfHost, denied);

  results.personhood_credential = await execute(
    selfHost,
    build("identity:personhood_status", [selfSigner.accountId]),
  );
  results.attestation_live_evidence = await execute(
    sponsorHost,
    build("attestation:attestation_live_status", [CREDENTIAL_ID]),
  );
  results.dot_lookup = await execute(
    selfHost,
    build("names:root_name_by_normalized_label", [normalizedLabel("festival")]),
  );
  results.dot_commit = await execute(selfHost, build("names:commit", [REGISTRATION_COMMITMENT]));
  results.dot_register = await execute(
    selfHost,
    build("names:register", [null, normalizedLabel("festival"), registrationSalt("festival-p6-salt")]),
  );
  results.dot_credential_link = await execute(selfHost, build("names:resolve_attestation", [NAME_ID]));

  const checkIn = {
    schema: SCHEMA_ID,
    subject_commitment: SUBJECT,
    payload_commitment: PAYLOAD,
    status_commitment: STATUS,
    parent: CREDENTIAL_ID,
    expiry: null,
    uniqueness_commitment: null,
    revocable: true,
  };
  const prepareCheckIn = async (nonce: string): Promise<Terminal> => execute(
    selfHost,
    build("transaction:prepare_sponsored_intent", [{
      participant: selfSigner.accountId,
      nonce,
      mortality: { valid_from: "100", valid_until: "164" },
      target: { capability: "attestation", method: "issue", payload: checkIn },
    }]),
  );
  const signPrepared = (prepared: Terminal): JsonObject => {
    assert.equal(prepared.code, "success");
    const envelope = prepared.response as JsonObject;
    const signingPayloadHash = String(envelope.signing_payload_hash);
    return {
      envelope,
      participant_signature: {
        scheme: "ed25519",
        value: `0x${signDetached(
          null,
          Buffer.from(signingPayloadHash.slice(2), "hex"),
          PARTICIPANT_PRIVATE_KEY,
        ).toString("hex")}`,
      },
    };
  };
  const submitSigned = async (signedIntent: JsonObject): Promise<Terminal> => execute(
    sponsorHost,
    build("transaction:submit_sponsored_intent", [signedIntent]),
  );
  const submitPrepared = async (prepared: Terminal): Promise<Terminal> => {
    return submitSigned(signPrepared(prepared));
  };
  results.prepare_sponsored_check_in = await prepareCheckIn("1");
  const signedCheckIn = signPrepared(results.prepare_sponsored_check_in);
  results.sponsored_check_in = await submitSigned(signedCheckIn);
  results.sponsored_replay = await submitSigned(signedCheckIn);
  const tamperedCheckIn = structuredClone(signedCheckIn);
  (tamperedCheckIn.envelope as JsonObject).nonce = "99";
  results.tampered_sponsored_intent = await submitSigned(tamperedCheckIn);
  results.prepare_exhausted_check_in = await prepareCheckIn("2");
  results.sponsor_budget_exhaustion = await submitPrepared(results.prepare_exhausted_check_in);

  state.online = false;
  const offlineRequest = build("names:root_name_by_normalized_label", [normalizedLabel("festival")]);
  results.offline = await execute(selfHost, offlineRequest);
  state.online = true;
  const reconnectRequest = build("names:root_name_by_normalized_label", [normalizedLabel("festival")]);
  results.offline_reconnect = await execute(selfHost, reconnectRequest);
  results.offline_replay = await execute(selfHost, reconnectRequest);

  state.selfPauseNext = true;
  const cancelledRequest = build("names:commit", [REGISTRATION_COMMITMENT]);
  const cancelling = execute(selfHost, cancelledRequest);
  await new Promise((done) => setTimeout(done, 0));
  selfHost.cancel(cancelledRequest.request_id);
  results.cancelled = await cancelling;
  results.cancel_reconnect = await execute(selfHost, build("names:commit", [REGISTRATION_COMMITMENT]));
  results.cancel_replay = await execute(selfHost, cancelledRequest);

  sponsorClient.identity = { ...sponsorClient.identity, spec_version: sponsorClient.identity.spec_version + 1 };
  results.version_drift = await execute(
    sponsorHost,
    build("attestation:attestation_live_status", [CREDENTIAL_ID]),
  );
  sponsorClient.identity = { ...sponsorClient.identity, spec_version: ORBIS_NETWORK_BINDING.spec_version };

  results.revoke_credential = await execute(sponsorHost, build("attestation:revoke", [CREDENTIAL_ID]));
  results.revoked_evidence = await execute(
    sponsorHost,
    build("attestation:attestation_live_status", [CREDENTIAL_ID]),
  );
  results.prepare_revoked_check_in = await prepareCheckIn("3");
  results.revoked_sponsored_denial = await submitPrepared(results.prepare_revoked_check_in);

  sponsorHost.revoke(APP_ID);
  results.permission_revoked = await execute(
    sponsorHost,
    build("attestation:attestation_live_status", [CREDENTIAL_ID]),
  );

  const expected: Record<string, string> = {
    permission_denial: "permission_denied",
    personhood_credential: "success",
    attestation_live_evidence: "success",
    dot_lookup: "success",
    dot_commit: "success",
    dot_register: "success",
    dot_credential_link: "success",
    prepare_sponsored_check_in: "success",
    sponsored_check_in: "success",
    sponsored_replay: "replay",
    tampered_sponsored_intent: "proof_invalid",
    prepare_exhausted_check_in: "success",
    sponsor_budget_exhaustion: "capacity_exceeded",
    offline: "timeout",
    offline_reconnect: "success",
    offline_replay: "conflict",
    cancelled: "cancelled",
    cancel_reconnect: "success",
    cancel_replay: "conflict",
    version_drift: "unsupported_runtime",
    revoke_credential: "success",
    revoked_evidence: "success",
    prepare_revoked_check_in: "success",
    revoked_sponsored_denial: "not_authorized",
    permission_revoked: "permission_revoked",
  };
  for (const [step, code] of Object.entries(expected)) assert.equal(results[step]?.code, code, step);
  assert.equal(results.offline.retryable, true);
  assert.ok(state.participantAuthorizations.every((value) => value.startsWith("participant-authorization:")));
  assert.ok(state.selfChainSigners.length >= 3 && state.selfChainSigners.every((value) => value === selfSigner.accountId));
  assert.ok(state.sponsorChainSigners.length >= 4 && state.sponsorChainSigners.every((value) => value === sponsorSigner.accountId));
  assert.notEqual(selfSigner.accountId, sponsorSigner.accountId);
  assert.deepEqual(state.events.map(({ event }) => event.event), [
    "name_registered",
    "sponsored_check_in",
    "attestation_revoked",
  ]);

  return {
    schema: "cord.festival-journey-report.v1",
    status: "PASS",
    journey_acceptance: true,
    p6_acceptance: false,
    application_id: APP_ID,
    network_activation: ORBIS_CANDIDATE_NETWORK_BINDING.activation_state,
    route_factory: "NATIVE_RUNTIME_ROUTE_REGISTRY",
    transport: "createTypedNetworkHostRoutes+FakeHost",
    results,
    signer_boundaries: {
      participant_authorization_count: state.participantAuthorizations.length,
      self_chain_signer: selfSigner.accountId,
      remote_sponsor_chain_signer: sponsorSigner.accountId,
      self_submission_count: state.selfChainSigners.length,
      sponsored_submission_count: state.sponsorRouteSelections.filter(
        (route) => route === "transaction:submit_sponsored_intent",
      ).length,
      distinct_chain_signers: true,
      qualification: "A deterministic Ed25519 participant key signs a digest committing the complete typed v7 envelope; the distinct host signer submits MetaTx::dispatch and requires finalized MetaTx::Dispatched(Ok). Runtime SCALE/signature parity remains covered by Rust integration tests.",
    },
    route_selections: {
      self: state.selfRouteSelections,
      sponsor: state.sponsorRouteSelections,
    },
    finalized_events: state.events,
    replay_boundaries: {
      sponsored_intent_replay: results.sponsored_replay.code,
      tampered_sponsored_intent: results.tampered_sponsored_intent.code,
      host_consent_replay: results.offline_replay.code,
      reconnect_uses_new_request_and_consent: true,
    },
    vector_observations: {
      "participant-dot-registration": {
        chain_signer: "participant",
        event: "name_registered",
      },
      "sponsored-check-in": {
        chain_signer: "sponsor",
        event: "sponsored_check_in",
        meta_tx_event: (results.sponsored_check_in.response as JsonObject).meta_tx_event,
        inner_result: (results.sponsored_check_in.response as JsonObject).inner_result,
      },
      reconnect: {
        new_request_and_consent: true,
      },
      "revoked-credential-deny": {
        event: "attestation_revoked",
      },
    },
    terminal_count: terminal.length,
    native_only: {
      raw_scale: false,
      contract_abi: false,
      pallet_indices: false,
      call_indices: false,
    },
    sealed_route_registry: {
      capabilities: ["attestation", "names", "identity", "storage", "transaction"],
      identity_routes: 9,
      personhood_routes: 1,
      sponsored_transaction_routes: 2,
    },
    production_evidence_deferred: [
      "Live-chain execution and production-finality observation remain outside this deterministic contract harness.",
      "Final E/Q/C SLO and storage-headroom campaigns run only after both P6 journeys are feature complete.",
      "Production Swift or Kotlin rewrites are explicit non-goals; mobile deliverables are contract harnesses.",
    ],
    mobile_contract_harness: {
      ios: "product-sdk/examples/festival/ios-contract-harness.manifest.json",
      android: "product-sdk/examples/festival/android-contract-harness.manifest.json",
      vectors: "product-sdk/examples/festival/mobile-contract-vectors.json",
    },
    deferred: {
      live_chain: true,
      production_finality: true,
      slo: true,
      production_mobile_rewrite: true,
    },
  };
}

const invokedPath = process.argv[1] ? resolve(process.argv[1]) : "";
if (invokedPath === fileURLToPath(import.meta.url)) {
  const report = await runFestivalJourney();
  if (process.argv.includes("--write")) {
    const path = resolve(import.meta.dirname, "../../../docs/evidence/verification/p6/festival-journey.report.json");
    mkdirSync(dirname(path), { recursive: true });
    writeFileSync(path, `${JSON.stringify(report, null, 2)}\n`);
    process.stdout.write(`${path}\n`);
  } else {
    process.stdout.write(`${JSON.stringify(report)}\n`);
  }
}
