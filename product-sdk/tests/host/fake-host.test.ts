import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import test from "node:test";
import { ProductSdkError, ERROR_CODES } from "../../packages/core/src/contract.ts";
import { assertCompositeSnapshot, FakeHost, type HostRequest } from "../../packages/host/src/fake-host.ts";
import { ORBIS_CANDIDATE_NETWORK_BINDING, ORBIS_NETWORK_BINDING } from "../../packages/descriptors/generated/orbis-network-binding.ts";

const GENESIS = ORBIS_NETWORK_BINDING.genesis_hash;
const ATTESTATION = `0x${"11".repeat(32)}`;

function req(overrides: any = {}): HostRequest {
  const base: HostRequest = {
    version: 1,
    request_id: "request-0000000001",
    application_id: "festival",
    capability: "attestation",
    method: "attestation_live_status",
    finality: "finalized",
    payload: { attestation: ATTESTATION },
    network: { ...ORBIS_CANDIDATE_NETWORK_BINDING },
    consent: { scope: ["attestation:attestation_live_status"], expires_at: 2_000, nonce: "nonce-00000000001" },
  };
  return {
    ...base,
    ...overrides,
    network: { ...base.network, ...overrides.network },
    consent: { ...base.consent, ...overrides.consent },
  };
}

function authorize(host: FakeHost, request: HostRequest): HostRequest {
  host.issueConsent(request.application_id, request.consent);
  return request;
}
async function code(operation: Promise<unknown>): Promise<string> {
  try { await operation; return "success"; } catch (error: any) { return error.code; }
}
function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => { resolve = done; });
  return { promise, resolve };
}

test("registered hostile suite executes typed outcomes", async () => {
  const registry = JSON.parse(readFileSync(resolve(import.meta.dirname, "../../../docs/sdk/host/fake-host-scenarios.json"), "utf8"));
  const out = new Map<string, string>();
  let host = new FakeHost();
  host.grant("festival", ["attestation:attestation_live_status"]);
  out.set("allow-scoped-read", await code(host.execute(authorize(host, req()))));

  out.set("deny-unknown-capability", await code(host.execute(req({
    request_id: "request-0000000002", capability: "unknown", method: "read",
    consent: { nonce: "nonce-00000000002", scope: ["unknown:read"] },
  }))));

  const ungranted = req({
    request_id: "request-0000000003", method: "schema_by_id", payload: { schema: ATTESTATION },
    consent: { nonce: "nonce-00000000003", scope: ["attestation:schema_by_id"] },
  });
  out.set("deny-scope-escalation", await code(host.execute(authorize(host, ungranted))));

	const missingConsent = req({ request_id: "request-missing-0001", consent: { nonce: "nonce-missing-00001" } });
	out.set("deny-missing-host-consent", await code(host.execute(missingConsent)));

	const selfAuthorized = req({ request_id: "request-selfauth-001", consent: { nonce: "nonce-selfauth-0001" } });
	host.issueConsent(selfAuthorized.application_id, selfAuthorized.consent);
	selfAuthorized.consent.scope.push("attestation:schema_by_id");
	out.set("deny-caller-self-authorized-consent", await code(host.execute(selfAuthorized)));

	const revokedConsent = authorize(host, req({ request_id: "request-consent-rev", consent: { nonce: "nonce-consent-revoke" } }));
	host.revokeConsent(revokedConsent.consent.nonce);
	out.set("deny-revoked-consent", await code(host.execute(revokedConsent)));

  const expired = req({ request_id: "request-0000000004", consent: { nonce: "nonce-00000000004", expires_at: 999 } });
  out.set("deny-expired-consent", await code(host.execute(authorize(host, expired))));

  host = new FakeHost();
  host.grant("festival", ["attestation:attestation_live_status"]);
  const replay = req({ consent: { nonce: "nonce-replay-00001" } });
  await host.execute(authorize(host, replay));
  out.set("deny-replayed-nonce", await code(host.execute(req({ request_id: "request-0000000005", consent: { nonce: "nonce-replay-00001" } }))));

  host = new FakeHost();
  host.grant("festival", ["attestation:attestation_live_status"]);
  const revoked = authorize(host, req());
  host.revoke("festival");
  out.set("revoke-before-sign", await code(host.execute(revoked)));

  host = new FakeHost();
  host.grant("festival", ["attestation:attestation_live_status"]);
  const cancelled = authorize(host, req());
  host.cancel(cancelled.request_id);
  out.set("cancel-in-flight", await code(host.execute(cancelled)));

  host = new FakeHost({ transport: async () => { throw new ProductSdkError("timeout", "timeout", true); } });
  host.grant("festival", ["attestation:attestation_live_status"]);
  out.set("transport-timeout", await code(host.execute(authorize(host, req()))));
  out.set("metadata-drift", await code(host.execute(req({ request_id: "request-0000000006", network: { spec_version: 30 } }))));
  try { assertCompositeSnapshot(["a", "b"]); out.set("composite-cross-hash", "success"); }
  catch (error: any) { out.set("composite-cross-hash", error.code); }
  out.set("contract-abi-request", await code(host.execute(req({ request_id: "request-0000000007", payload: { attestation: ATTESTATION, nested: { ABI: [] } } }))));

  assert.deepEqual([...out.keys()].sort(), registry.scenarios.map((item: any) => item.id).sort());
  const expected: Record<string, string> = {
    "allow-scoped-read": "success",
    "deny-unknown-capability": "unsupported_surface",
    "deny-scope-escalation": "permission_denied",
		"deny-missing-host-consent": "permission_denied",
		"deny-caller-self-authorized-consent": "permission_denied",
		"deny-revoked-consent": "permission_revoked",
    "deny-expired-consent": "consent_expired",
    "deny-replayed-nonce": "replay",
    "revoke-before-sign": "permission_revoked",
    "cancel-in-flight": "cancelled",
    "transport-timeout": "timeout",
    "metadata-drift": "unsupported_runtime",
    "composite-cross-hash": "inconsistent_snapshot",
    "contract-abi-request": "unsupported_surface",
  };
  for (const [id, value] of out) assert.equal(value, expected[id], id);
});

test("host-owned consent rejects missing and revoked records", async () => {
  const host = new FakeHost();
  host.grant("festival", ["attestation:attestation_live_status"]);
  assert.equal(await code(host.execute(req())), "permission_denied");
  const request = authorize(host, req({ request_id: "request-consent-revoke", consent: { nonce: "nonce-consent-revoke" } }));
  host.revokeConsent(request.consent.nonce);
  assert.equal(await code(host.execute(request)), "permission_revoked");
});

test("grant and consent issuance reject unknown exact scopes with Rust parity", () => {
	const host = new FakeHost();
	assert.throws(() => host.grant("festival", ["attestation:unknown"]), (error: ProductSdkError) => error.code === "invalid_input");
	assert.throws(() => host.issueConsent("festival", { scope: ["attestation:unknown"], expires_at: 2_000, nonce: "nonce-unknown-0001" }), (error: ProductSdkError) => error.code === "invalid_input");
});

test("deferred cancellation wins once and late transport cannot callback", async () => {
  const transport = deferred<{ finalizedHash: string }>();
  const terminal: string[] = [];
  const host = new FakeHost({ transport: async () => transport.promise, onTerminal: (_, outcome) => terminal.push(outcome) });
  host.grant("festival", ["attestation:attestation_live_status"]);
  const request = authorize(host, req());
  const pending = host.execute(request);
  await new Promise((done) => setTimeout(done, 0));
  host.cancel(request.request_id); host.cancel(request.request_id);
  assert.equal(await code(pending), "cancelled");
  transport.resolve({ finalizedHash: `0x${"aa".repeat(32)}` });
  await new Promise((done) => setTimeout(done, 0));
  assert.deepEqual(terminal, ["cancelled"]);
});

test("revocation is checked before signer and between signer and transport", async () => {
  let signs = 0, transports = 0;
  let host = new FakeHost({ signer: async () => { signs++; return "sig"; }, transport: async () => { transports++; return { finalizedHash: "x" }; } });
  host.grant("festival", ["attestation:attestation_live_status"]);
  const before = authorize(host, req()); host.revoke("festival");
  assert.equal(await code(host.execute(before)), "permission_revoked");
  assert.deepEqual([signs, transports], [0, 0]);

  const signature = deferred<string>();
  host = new FakeHost({ signer: async () => { signs++; return signature.promise; }, transport: async () => { transports++; return { finalizedHash: "x" }; } });
  host.grant("festival", ["attestation:attestation_live_status"]);
  const request = authorize(host, req({ request_id: "request-0000000002", consent: { nonce: "nonce-00000000002" } }));
  const pending = host.execute(request);
  await new Promise((done) => setTimeout(done, 0));
  host.revoke("festival"); signature.resolve("sig");
  assert.equal(await code(pending), "permission_revoked");
  assert.equal(transports, 0);

  const consentSignature = deferred<string>();
  host = new FakeHost({ signer: async () => consentSignature.promise, transport: async () => { transports++; return { finalizedHash: "x" }; } });
  host.grant("festival", ["attestation:attestation_live_status"]);
  const consentRequest = authorize(host, req({ request_id: "request-consent-delay", consent: { nonce: "nonce-consent-delay" } }));
  const consentPending = host.execute(consentRequest);
  await new Promise((done) => setTimeout(done, 0));
  host.revokeConsent(consentRequest.consent.nonce); consentSignature.resolve("sig");
  assert.equal(await code(consentPending), "permission_revoked");
  assert.equal(transports, 0);
  assert.equal(await code(host.execute(req({ request_id: "request-consent-replay", consent: { nonce: consentRequest.consent.nonce } }))), "replay");
});

test("candidate network requires explicit opt-in and production mode rejects PENDING activation", async () => {
  const candidateHost = new FakeHost();
  candidateHost.grant("festival", ["attestation:attestation_live_status"]);
  assert.equal(await code(candidateHost.execute(authorize(candidateHost, req()))), "success");

  const productionHost = new FakeHost();
  productionHost.grant("festival", ["attestation:attestation_live_status"]);
  const production = req({
    request_id: "request-production-001",
    network: { ...ORBIS_NETWORK_BINDING, access_mode: "production" },
    consent: { nonce: "nonce-production-001" },
  });
  assert.equal(await code(productionHost.execute(authorize(productionHost, production))), "unsupported_runtime");
});

test("every thrown error serializes to native-error-v1 shape", () => {
  const schema = JSON.parse(readFileSync(resolve(import.meta.dirname, "../../../docs/sdk/native-error.schema.json"), "utf8"));
  const error = new ProductSdkError("timeout", "typed timeout", true, { stage: "transport" }).toJSON();
  assert.deepEqual(Object.keys(error).sort(), ["code", "details", "message", "retryable", "version"]);
  assert.ok(schema.properties.code.enum.includes(error.code));
  assert.deepEqual(ERROR_CODES, schema.properties.code.enum);
});

test("host JSON contract is method-discriminated, recursively closed and exactly runtime-bound", () => {
  const schema = JSON.parse(readFileSync(resolve(import.meta.dirname, "../../../docs/sdk/host/host-request.schema.json"), "utf8"));
  const descriptor = JSON.parse(readFileSync(resolve(import.meta.dirname, "../../packages/descriptors/generated/orbis-descriptor.json"), "utf8"));
  assert.equal(schema.additionalProperties, false);
  assert.equal(schema.oneOf.length, descriptor.nativeHostContract.methodCount);
  for (const branch of schema.oneOf) assert.equal(branch.properties.payload.additionalProperties, false);
  const network = schema.properties.network;
  assert.equal(network.additionalProperties, false);
  for (const key of ["genesis_hash", "metadata_hash", "descriptor_contract_sha256", "chain_spec_source_sha256", "spec_version", "transaction_version", "activation_state", "production_activation_ready", "access_mode"]) assert.ok(network.required.includes(key));
  assert.equal(network.properties.genesis_hash.const, GENESIS);
  assert.equal(network.properties.descriptor_contract_sha256.const, ORBIS_NETWORK_BINDING.descriptor_contract_sha256);
});

test("duplicate and concurrent request IDs fail before a second signer or transport", async () => {
  const signature = deferred<string>();
  let signs = 0, transports = 0;
  const terminal: string[] = [];
  const host = new FakeHost({ signer: async () => { signs++; return signature.promise; }, transport: async () => { transports++; return { finalizedHash: "x" }; }, onTerminal: (_, outcome) => terminal.push(outcome) });
  host.grant("festival", ["attestation:attestation_live_status"]);
  const request = authorize(host, req());
  const first = host.execute(request);
  await new Promise((done) => setTimeout(done, 0));
  assert.equal(await code(host.execute(request)), "conflict");
  assert.deepEqual([signs, transports], [1, 0]);
  signature.resolve("sig");
  assert.equal(await code(first), "success");
  assert.equal(await code(host.execute(request)), "conflict");
  assert.deepEqual([signs, transports], [1, 1]);
  assert.deepEqual(terminal, ["success"]);
});

test("manual Draft-2020-12 boundary rejects malformed fields", async () => {
  const mutations = [
    (value: any) => { value.network.spec_version = "29"; },
    (value: any) => { value.consent.expires_at = "2000"; },
    (value: any) => { value.finality = true; },
    (value: any) => { value.version = "1"; },
    (value: any) => { value.network.extra = false; },
    (value: any) => { value.consent.scope = ["attestation:attestation_live_status", 1]; },
  ];
  for (const [index, mutate] of mutations.entries()) {
    const request: any = req({ request_id: `request-malformed-${String(index).padStart(4, "0")}` });
    mutate(request);
    const host = new FakeHost(); host.grant("festival", ["attestation:attestation_live_status"]);
    assert.equal(await code(host.execute(request)), "invalid_input");
  }
});
