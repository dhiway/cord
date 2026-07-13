import assert from "node:assert/strict";
import test from "node:test";
import { ORBIS_NETWORK_BINDING } from "../../packages/descriptors/generated/orbis-network-binding.ts";
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
    capability: "identity",
    method: "read",
    network: { ...ORBIS_NETWORK_BINDING },
    finality: "finalized",
    payload: { subject_id: "subject:alice" },
    consent: {
      scope: ["identity:read"],
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
    "identity:read": {
      finality: "finalized",
      async query(payload: JsonObject, context: { at: string }) {
        queriedAt.push(context.at);
        return { subject_id: payload.subject_id, active: true };
      },
    },
  } satisfies TypedNetworkRoutes<DeterministicTypedClient, typeof chainSigner>;
  const host = new FakeHost({
    ...createTypedNetworkHostRoutes({ client, signer: chainSigner, binding: ORBIS_NETWORK_BINDING, routes }),
  });
  host.grant("festival", ["identity"]);

  const result = await host.execute(request());
  assert.equal(result.finalizedHash, INITIAL_HASH);
  assert.deepEqual(result.response, { subject_id: "subject:alice", active: true });
  assert.deepEqual(queriedAt, [INITIAL_HASH]);
  assert.deepEqual(client.runtimeIdentityCalls, [INITIAL_HASH]);
});

test("metadata-derived submission resolves only after typed finalization evidence", async () => {
  const client = new DeterministicTypedClient();
  const constructedAt: string[] = [];
  const signedBy: string[] = [];
  const routes = {
    "transaction:submit": {
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
  host.grant("festival", ["transaction"]);

  const result = await host.execute(request({
    capability: "transaction",
    method: "submit",
    finality: "submit-and-finalize",
    payload: { operation_id: "native-operation", intent_id: "intent-0000000001" },
    consent: { scope: ["transaction:submit"], expires_at: 2_000, nonce: "submit-consent-00000001" },
  }));

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
    "identity:read": {
      finality: "finalized",
      async query() { queries++; return null; },
    },
  } satisfies TypedNetworkRoutes<DeterministicTypedClient, typeof chainSigner>;
  const host = new FakeHost({
    ...createTypedNetworkHostRoutes({ client, signer: chainSigner, binding: ORBIS_NETWORK_BINDING, routes }),
  });
  host.grant("festival", ["identity"]);

  assert.equal(await errorCode(host.execute(request())), "metadata_mismatch");
  assert.equal(queries, 0);
});

test("host cancellation aborts and closes an in-flight typed transaction stream", async () => {
  const client = new DeterministicTypedClient();
  let closed = false;
  const routes = {
    "transaction:submit": {
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
  host.grant("festival", ["transaction"]);
  const pending = host.execute(request({
    capability: "transaction",
    method: "submit",
    finality: "submit-and-finalize",
    payload: { operation_id: "native-operation", intent_id: "intent-0000000001" },
    consent: { scope: ["transaction:submit"], expires_at: 2_000, nonce: "cancel-consent-00000001" },
  }));
  await new Promise((resolve) => setTimeout(resolve, 0));
  host.cancel("network-request-00000001");

  assert.equal(await errorCode(pending), "cancelled");
  await new Promise((resolve) => setTimeout(resolve, 0));
  assert.equal(closed, true);
});

test("typed client failures map to the stable product error vocabulary", async () => {
  const client = new DeterministicTypedClient();
  const routes = {
    "identity:read": {
      finality: "finalized",
      async query(): Promise<never> {
        throw { code: "network", message: "endpoint unavailable", retryable: true };
      },
    },
  } satisfies TypedNetworkRoutes<DeterministicTypedClient, typeof chainSigner>;
  const host = new FakeHost({
    ...createTypedNetworkHostRoutes({ client, signer: chainSigner, binding: ORBIS_NETWORK_BINDING, routes }),
  });
  host.grant("festival", ["identity"]);

  assert.equal(await errorCode(host.execute(request())), "timeout");
});
