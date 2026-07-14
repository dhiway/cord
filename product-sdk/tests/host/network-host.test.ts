import assert from "node:assert/strict";
import test from "node:test";
import { ORBIS_CANDIDATE_NETWORK_BINDING, ORBIS_NETWORK_BINDING } from "../../packages/descriptors/generated/orbis-network-binding.ts";
import { ProductSdkError, type JsonObject } from "../../packages/core/src/contract.ts";
import { FakeHost, type HostRequest } from "../../packages/host/src/fake-host.ts";
import {
  createTypedNetworkHostRoutes,
  type TypedNetworkRoutes,
  type TypedPapiClient,
  type TypedRuntimeIdentity,
} from "../../packages/host/src/network-host.ts";

const INITIAL_HASH = `0x${"11".repeat(32)}`;
const FINAL_HASH = `0x${"22".repeat(32)}`;
const EXTRINSIC_HASH = `0x${"33".repeat(32)}`;

class DeterministicTypedClient implements TypedPapiClient {
  readonly runtimeIdentityCalls: string[] = [];
  finalizedHash = INITIAL_HASH;
  identity: TypedRuntimeIdentity = {
    genesis_hash: ORBIS_NETWORK_BINDING.genesis_hash,
    spec_version: ORBIS_NETWORK_BINDING.spec_version,
    transaction_version: ORBIS_NETWORK_BINDING.transaction_version,
    metadata_hash: ORBIS_NETWORK_BINDING.metadata_hash,
  };

  async getFinalizedBlock(): Promise<{ hash: string }> {
    return { hash: this.finalizedHash };
  }

  async getRuntimeIdentityAt(hash: string): Promise<TypedRuntimeIdentity> {
    this.runtimeIdentityCalls.push(hash);
    return this.identity;
  }
}

const chainSigner = { accountId: "provider:alice" } as const;

function request(overrides: Partial<HostRequest> = {}): HostRequest {
  const base: HostRequest = {
    version: 1,
    request_id: "network-request-00000001",
    application_id: "festival",
    capability: "attestation",
    method: "attestation_live_status",
    network: { ...ORBIS_CANDIDATE_NETWORK_BINDING },
    finality: "finalized",
    payload: { attestation: `0x${"44".repeat(32)}` },
    consent: {
      scope: ["attestation:attestation_live_status"],
      expires_at: 2_000,
      nonce: "network-consent-00000001",
    },
  };
  return {
    ...base,
    ...overrides,
    network: { ...base.network, ...overrides.network },
    consent: { ...base.consent, ...overrides.consent },
  };
}

function authorize(host: FakeHost, value: HostRequest): HostRequest {
  host.issueConsent(value.application_id, value.consent);
  return value;
}

async function errorCode(operation: Promise<unknown>): Promise<string> {
  try {
    await operation;
    return "success";
  } catch (error) {
    assert.ok(error instanceof ProductSdkError);
    return error.code;
  }
}

test("typed adapter executes a read at one exact finalized hash", async () => {
  const client = new DeterministicTypedClient();
  const queriedAt: string[] = [];
  const routes = {
    "attestation:attestation_live_status": {
      finality: "finalized",
      async query(payload: JsonObject, context: { at: string }) {
        queriedAt.push(context.at);
        return { attestation: payload.attestation, active: true };
      },
    },
  } satisfies TypedNetworkRoutes<DeterministicTypedClient, typeof chainSigner>;
  const host = new FakeHost({
    ...createTypedNetworkHostRoutes({ client, signer: chainSigner, binding: ORBIS_NETWORK_BINDING, routes }),
  });
  host.grant("festival", ["attestation:attestation_live_status"]);

  const result = await host.execute(authorize(host, request()));
  assert.equal(result.finalizedHash, INITIAL_HASH);
  assert.deepEqual(result.response, { attestation: `0x${"44".repeat(32)}`, active: true });
  assert.deepEqual(queriedAt, [INITIAL_HASH]);
  assert.deepEqual(client.runtimeIdentityCalls, [INITIAL_HASH]);
});

test("metadata-derived submission resolves only after typed finalization evidence", async () => {
  const client = new DeterministicTypedClient();
  const constructedAt: string[] = [];
  const signedBy: string[] = [];
  const routes = {
    "dotns:commit": {
      finality: "submit-and-finalize",
      transaction(_payload: JsonObject, context: { at: string }) {
        constructedAt.push(context.at);
        return {
          async *signSubmitAndWatch(signer: typeof chainSigner) {
            signedBy.push(signer.accountId);
            yield { type: "broadcasted" as const };
            yield { type: "included" as const, blockHash: FINAL_HASH };
            yield { type: "finalized" as const, blockHash: FINAL_HASH, extrinsicHash: EXTRINSIC_HASH };
          },
        };
      },
    },
  } satisfies TypedNetworkRoutes<DeterministicTypedClient, typeof chainSigner>;
  const host = new FakeHost({
    ...createTypedNetworkHostRoutes({ client, signer: chainSigner, binding: ORBIS_NETWORK_BINDING, routes }),
  });
  host.grant("festival", ["dotns:commit"]);

  const submission = request({
    capability: "dotns",
    method: "commit",
    finality: "submit-and-finalize",
    payload: { commitment: `0x${"55".repeat(32)}` },
    consent: { scope: ["dotns:commit"], expires_at: 2_000, nonce: "submit-consent-00000001" },
  });
  const result = await host.execute(authorize(host, submission));

  assert.deepEqual(constructedAt, [INITIAL_HASH]);
  assert.deepEqual(signedBy, [chainSigner.accountId]);
  assert.equal(result.finalizedHash, FINAL_HASH);
  assert.equal(result.extrinsicHash, EXTRINSIC_HASH);
  assert.deepEqual(result.lifecycle, {
    version: 1,
    intent_id: "network-request-00000001",
    state: "finalized",
    block_hash: FINAL_HASH,
    extrinsic_hash: EXTRINSIC_HASH,
  });
  assert.deepEqual(client.runtimeIdentityCalls, [INITIAL_HASH, FINAL_HASH]);
});

test("runtime drift fails closed before a typed read executes", async () => {
  const client = new DeterministicTypedClient();
  client.identity = { ...client.identity, metadata_hash: `0x${"44".repeat(32)}` };
  let queries = 0;
  const routes = {
    "attestation:attestation_live_status": {
      finality: "finalized",
      async query() { queries++; return null; },
    },
  } satisfies TypedNetworkRoutes<DeterministicTypedClient, typeof chainSigner>;
  const host = new FakeHost({
    ...createTypedNetworkHostRoutes({ client, signer: chainSigner, binding: ORBIS_NETWORK_BINDING, routes }),
  });
  host.grant("festival", ["attestation:attestation_live_status"]);

  assert.equal(await errorCode(host.execute(authorize(host, request()))), "metadata_mismatch");
  assert.equal(queries, 0);
});

test("host cancellation aborts and closes an in-flight typed transaction stream", async () => {
  const client = new DeterministicTypedClient();
  let closed = false;
  const routes = {
    "dotns:commit": {
      finality: "submit-and-finalize",
      transaction() {
        return {
          async *signSubmitAndWatch(_signer: typeof chainSigner, options: { signal: AbortSignal }) {
            try {
              yield { type: "broadcasted" as const };
              await new Promise<never>((_, reject) =>
                options.signal.addEventListener("abort", () => reject(new Error("aborted")), { once: true }));
            } finally {
              closed = true;
            }
          },
        };
      },
    },
  } satisfies TypedNetworkRoutes<DeterministicTypedClient, typeof chainSigner>;
  const host = new FakeHost({
    ...createTypedNetworkHostRoutes({ client, signer: chainSigner, binding: ORBIS_NETWORK_BINDING, routes }),
  });
  host.grant("festival", ["dotns:commit"]);
  const cancellable = request({
    capability: "dotns",
    method: "commit",
    finality: "submit-and-finalize",
    payload: { commitment: `0x${"55".repeat(32)}` },
    consent: { scope: ["dotns:commit"], expires_at: 2_000, nonce: "cancel-consent-00000001" },
  });
  const pending = host.execute(authorize(host, cancellable));
  await new Promise((resolve) => setTimeout(resolve, 0));
  host.cancel("network-request-00000001");

  assert.equal(await errorCode(pending), "cancelled");
  await new Promise((resolve) => setTimeout(resolve, 0));
  assert.equal(closed, true);
});

test("typed client failures map to the stable product error vocabulary", async () => {
  const client = new DeterministicTypedClient();
  const routes = {
    "attestation:attestation_live_status": {
      finality: "finalized",
      async query(): Promise<never> {
        throw { code: "network", message: "endpoint unavailable", retryable: true };
      },
    },
  } satisfies TypedNetworkRoutes<DeterministicTypedClient, typeof chainSigner>;
  const host = new FakeHost({
    ...createTypedNetworkHostRoutes({ client, signer: chainSigner, binding: ORBIS_NETWORK_BINDING, routes }),
  });
  host.grant("festival", ["attestation:attestation_live_status"]);

  assert.equal(await errorCode(host.execute(authorize(host, request()))), "timeout");
});
