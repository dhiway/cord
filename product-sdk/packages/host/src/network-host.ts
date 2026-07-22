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
  ERROR_CODES,
  ProductSdkError,
  type ErrorCode,
  type JsonObject,
  type JsonValue,
} from "../../core/src/contract.ts";
import type {
  HostDependencies,
  HostRoute,
  HostTransportResult,
  SignedRequest,
} from "./fake-host.ts";

export { ORBIS_CANDIDATE_NETWORK_BINDING, ORBIS_NETWORK_BINDING } from "../../descriptors/generated/orbis-network-binding.ts";

export interface TypedFinalizedBlock {
  readonly hash: string;
  /** Canonical decimal `u32` block number paired with `hash`. */
  readonly number: string;
}

/** Runtime identity decoded by the typed client at one exact block hash. */
export interface TypedRuntimeIdentity {
  readonly genesis_hash: string;
  readonly spec_version: number;
  readonly transaction_version: number;
  readonly metadata_hash: string;
}

/**
 * Minimal PAPI-like client boundary required by the product host.
 *
 * Implementations are expected to be descriptor generated. No raw storage key,
 * SCALE byte, pallet index, call index, or RPC `state_call` escape hatch is
 * accepted by this interface.
 */
export interface TypedPapiClient {
  getFinalizedBlock(signal: AbortSignal): Promise<TypedFinalizedBlock>;
  getRuntimeIdentityAt(hash: string, signal: AbortSignal): Promise<TypedRuntimeIdentity>;
}

export interface TypedChainSigner {
  readonly accountId: string;
}

export interface TypedFinalizedEvent {
  readonly pallet: string;
  readonly event: string;
  readonly fields: JsonObject;
}

export interface TypedReadContext<Client extends TypedPapiClient> {
  readonly client: Client;
  readonly at: string;
  readonly atNumber: string;
  readonly signal: AbortSignal;
  /** Signature over the already validated host request, for audit/policy bindings. */
  readonly authorizationSignature: string;
}

export interface TypedTransactionContext<Client extends TypedPapiClient> {
  readonly client: Client;
  /** Finalized block whose metadata/runtime identity authorized construction. */
  readonly at: string;
  readonly atNumber: string;
  readonly authorizationSignature: string;
}

export type TypedTransactionStatus =
  | { readonly type: "broadcasted" }
  | { readonly type: "included"; readonly blockHash: string; readonly extrinsicHash?: string }
  | {
      readonly type: "finalized";
      readonly blockHash: string;
      readonly extrinsicHash: string;
      readonly events?: readonly TypedFinalizedEvent[];
    }
  | { readonly type: "rejected"; readonly error: TypedClientFailure };

/** A descriptor-generated transaction; its implementation owns encoding. */
export interface TypedPapiTransaction<Signer extends TypedChainSigner> {
  signSubmitAndWatch(
    signer: Signer,
    options: { readonly signal: AbortSignal },
  ): AsyncIterable<TypedTransactionStatus>;
}

export interface TypedClientFailure {
  readonly code?: ErrorCode | "dispatch_error" | "invalid" | "dropped" | "usurped" | "network";
  readonly message: string;
  readonly retryable?: boolean;
  readonly details?: JsonObject;
}

export interface TypedFinalizedReadRoute<Client extends TypedPapiClient> {
  readonly finality: "finalized";
  query(payload: JsonObject, context: TypedReadContext<Client>): Promise<JsonValue>;
}

export interface TypedSubmitRoute<
  Client extends TypedPapiClient,
  Signer extends TypedChainSigner,
> {
  readonly finality: "submit-and-finalize";
  /** Build through generated metadata descriptors, never pallet/call indices. */
  transaction(
    payload: JsonObject,
    context: TypedTransactionContext<Client>,
  ): TypedPapiTransaction<Signer>;
}

export type TypedNetworkRoute<
  Client extends TypedPapiClient,
  Signer extends TypedChainSigner,
> = TypedFinalizedReadRoute<Client> | TypedSubmitRoute<Client, Signer>;

export type TypedNetworkRoutes<
  Client extends TypedPapiClient,
  Signer extends TypedChainSigner,
> = Readonly<Record<string, TypedNetworkRoute<Client, Signer>>>;

/**
 * Required descriptor-v8 boundary for sponsored intents.
 *
 * The SDK deliberately has no fallback encoder. An implementation must construct the active
 * metadata-v8 call and participant signing payload through generated typed APIs. Submission must
 * surface decoded finalized events; raw SCALE, pallet indices and call indices are not accepted.
 */
export interface TypedSponsoredIntentTransport<
  Client extends TypedPapiClient,
  Signer extends TypedChainSigner,
> {
  prepare(payload: JsonObject, context: TypedReadContext<Client>): Promise<JsonValue>;
  submit(payload: JsonObject, context: TypedTransactionContext<Client>): TypedPapiTransaction<Signer>;
}

export function createTypedSponsoredIntentRoutes<
  Client extends TypedPapiClient,
  Signer extends TypedChainSigner,
>(transport: TypedSponsoredIntentTransport<Client, Signer>): TypedNetworkRoutes<Client, Signer> {
  return {
    "transaction:prepare_sponsored_intent": {
      finality: "finalized",
      query: (payload, context) => transport.prepare(payload, context),
    },
    "transaction:submit_sponsored_intent": {
      finality: "submit-and-finalize",
      transaction: (payload, context) => transport.submit(payload, context),
    },
  };
}

export interface NetworkBindingContract {
  readonly genesis_hash: string;
  readonly spec_version: number;
  readonly transaction_version: number;
  readonly metadata_hash: string;
  readonly descriptor_contract_sha256: string;
  readonly chain_spec_source_sha256: string;
  readonly activation_state: "candidate-pending" | "production-approved";
  readonly production_activation_ready: boolean;
}

export interface TypedNetworkHostOptions<
  Client extends TypedPapiClient,
  Signer extends TypedChainSigner,
> {
  readonly client: Client;
  readonly signer: Signer;
  /** Generated descriptor/network binding supplied by the CORD build. */
  readonly binding: NetworkBindingContract;
  /** Keys are exact `capability:method` host routes. */
  readonly routes: TypedNetworkRoutes<Client, Signer>;
}

const HASH_32 = /^0x[0-9a-f]{64}$/i;
const SHA_256 = /^[0-9a-f]{64}$/i;
const DECIMAL_U32 = /^(0|[1-9][0-9]{0,9})$/;

function assertHash(value: string, label: string): void {
  if (!HASH_32.test(value)) throw new ProductSdkError("invalid_input", `${label} must be a 32-byte hash`);
}

function assertBlockNumber(value: string, label: string): void {
  if (!DECIMAL_U32.test(value) || BigInt(value) > 0xffff_ffffn)
    throw new ProductSdkError("invalid_input", `${label} must be a canonical u32 decimal string`);
}

function assertBindingShape(binding: NetworkBindingContract): void {
  assertHash(binding.genesis_hash, "binding genesis_hash");
  assertHash(binding.metadata_hash, "binding metadata_hash");
  if (!SHA_256.test(binding.descriptor_contract_sha256))
    throw new ProductSdkError("invalid_input", "binding descriptor digest must be SHA-256");
  if (!SHA_256.test(binding.chain_spec_source_sha256))
    throw new ProductSdkError("invalid_input", "binding chain-spec digest must be SHA-256");
  if (!Number.isSafeInteger(binding.spec_version) || binding.spec_version < 0 ||
      !Number.isSafeInteger(binding.transaction_version) || binding.transaction_version < 0)
    throw new ProductSdkError("invalid_input", "binding runtime versions must be non-negative integers");
  if (!(["candidate-pending", "production-approved"] as unknown[]).includes(binding.activation_state)
    || typeof binding.production_activation_ready !== "boolean")
    throw new ProductSdkError("invalid_input", "binding activation state is invalid");
}

function equalBinding(actual: NetworkBindingContract, expected: NetworkBindingContract): void {
  if (actual.genesis_hash !== expected.genesis_hash)
    throw new ProductSdkError("unsupported_runtime", "genesis hash does not match the configured network");
  if (actual.spec_version !== expected.spec_version ||
      actual.transaction_version !== expected.transaction_version)
    throw new ProductSdkError(
      "unsupported_runtime",
      `runtime ${actual.spec_version}/${actual.transaction_version} does not match the configured network`,
    );
  if (actual.metadata_hash !== expected.metadata_hash)
    throw new ProductSdkError("metadata_mismatch", "metadata hash does not match the generated descriptor");
  if (actual.descriptor_contract_sha256 !== expected.descriptor_contract_sha256)
    throw new ProductSdkError("descriptor_mismatch", "descriptor contract digest mismatch");
  if (actual.chain_spec_source_sha256 !== expected.chain_spec_source_sha256)
    throw new ProductSdkError("unsupported_runtime", "chain-spec source digest mismatch");
  if (actual.activation_state !== expected.activation_state
    || actual.production_activation_ready !== expected.production_activation_ready)
    throw new ProductSdkError("unsupported_runtime", "network activation state mismatch");
}

function equalObservedRuntime(
  observed: TypedRuntimeIdentity,
  expected: NetworkBindingContract,
): void {
  if (observed.genesis_hash !== expected.genesis_hash)
    throw new ProductSdkError("unsupported_runtime", "typed client is connected to another genesis");
  if (observed.spec_version !== expected.spec_version ||
      observed.transaction_version !== expected.transaction_version)
    throw new ProductSdkError(
      "unsupported_runtime",
      `typed client observed runtime ${observed.spec_version}/${observed.transaction_version}`,
    );
  if (observed.metadata_hash !== expected.metadata_hash)
    throw new ProductSdkError("metadata_mismatch", "typed client observed a different metadata hash");
}

function assertJsonValue(value: unknown, path = "response"): asserts value is JsonValue {
  if (value === null || typeof value === "string" || typeof value === "boolean") return;
  if (typeof value === "number") {
    if (!Number.isFinite(value)) throw new ProductSdkError("runtime_rejected", `${path} is not JSON-safe`);
    return;
  }
  if (Array.isArray(value)) {
    value.forEach((child, index) => assertJsonValue(child, `${path}[${index}]`));
    return;
  }
  if (value && typeof value === "object") {
    for (const [key, child] of Object.entries(value)) assertJsonValue(child, `${path}.${key}`);
    return;
  }
  throw new ProductSdkError("runtime_rejected", `${path} is not JSON-safe`);
}

function validatePreparedSponsoredIntent(
  response: JsonValue,
  requestPayload: JsonObject,
  binding: NetworkBindingContract,
): asserts response is JsonObject {
  const keys = [
    "version", "signing_domain", "genesis_hash", "spec_version", "transaction_version",
    "metadata_hash", "participant", "nonce", "mortality", "target",
    "signing_payload_hash", "intent_id",
  ];
  if (!response || typeof response !== "object" || Array.isArray(response)
    || Object.keys(response).sort().join() !== keys.sort().join()
    || response.version !== 1
    || response.signing_domain !== "orbis/meta-intent/v7"
    || response.genesis_hash !== binding.genesis_hash
    || response.spec_version !== binding.spec_version
    || response.transaction_version !== binding.transaction_version
    || response.metadata_hash !== binding.metadata_hash
    || response.participant !== requestPayload.participant
    || response.nonce !== requestPayload.nonce
    || JSON.stringify(response.mortality) !== JSON.stringify(requestPayload.mortality)
    || JSON.stringify(response.target) !== JSON.stringify(requestPayload.target)
    || typeof response.signing_payload_hash !== "string" || !HASH_32.test(response.signing_payload_hash)
    || typeof response.intent_id !== "string" || !HASH_32.test(response.intent_id)) {
    throw new ProductSdkError(
      "runtime_rejected",
      "typed v8 transport returned an invalid sponsored signing envelope",
    );
  }
}

function cancelled(): ProductSdkError {
  return new ProductSdkError("cancelled", "network operation cancelled");
}

function throwIfAborted(signal: AbortSignal): void {
  if (signal.aborted) throw cancelled();
}

async function abortable<T>(operation: Promise<T>, signal: AbortSignal): Promise<T> {
  throwIfAborted(signal);
  let onAbort!: () => void;
  const aborted = new Promise<never>((_, reject) => {
    onAbort = () => reject(cancelled());
    signal.addEventListener("abort", onAbort, { once: true });
  });
  try {
    return await Promise.race([operation, aborted]);
  } finally {
    signal.removeEventListener("abort", onAbort);
  }
}

function routeFor<Client extends TypedPapiClient, Signer extends TypedChainSigner>(
  routes: TypedNetworkRoutes<Client, Signer>,
  request: SignedRequest["request"],
): TypedNetworkRoute<Client, Signer> {
  const route = routes[`${request.capability}:${request.method}`];
  if (!route)
    throw new ProductSdkError(
      "unsupported_surface",
      `typed network adapter has no route for ${request.capability}.${request.method}`,
    );
  if (route.finality !== request.finality)
    throw new ProductSdkError("invalid_input", "typed route finality does not match the request");
  return route;
}

function mapFailure(error: unknown): ProductSdkError {
  if (error instanceof ProductSdkError) return error;
  if (error && typeof error === "object") {
    const failure = error as Partial<TypedClientFailure>;
    const message = typeof failure.message === "string" && failure.message
      ? failure.message.slice(0, 512)
      : "typed client rejected the operation";
    const details = failure.details && typeof failure.details === "object" ? failure.details : {};
    if (failure.code && ERROR_CODES.includes(failure.code as ErrorCode))
      return new ProductSdkError(
        failure.code as ErrorCode,
        message,
        failure.retryable ?? failure.code === "timeout",
        details,
      );
    if (["invalid", "dropped", "usurped", "dispatch_error"].includes(String(failure.code)))
      return new ProductSdkError("runtime_rejected", message, false, details);
    if (failure.code === "network") return new ProductSdkError("timeout", message, true, details);
  }
  return new ProductSdkError("runtime_rejected", "typed client failed without a recognized error");
}

async function finalizedContext<Client extends TypedPapiClient>(
  client: Client,
  expected: NetworkBindingContract,
  signal: AbortSignal,
): Promise<TypedFinalizedBlock> {
  const finalized = await abortable(client.getFinalizedBlock(signal), signal);
  assertHash(finalized.hash, "finalized block hash");
  assertBlockNumber(finalized.number, "finalized block number");
  const observed = await abortable(client.getRuntimeIdentityAt(finalized.hash, signal), signal);
  equalObservedRuntime(observed, expected);
  return finalized;
}
function assertSponsoredPreparationAnchor(payload: JsonObject, finalized: TypedFinalizedBlock): void {
  const mortality = payload.mortality as JsonObject;
  if (mortality.valid_from !== finalized.number) {
    throw new ProductSdkError(
      "invalid_input",
      "sponsored mortality valid_from must equal the finalized preparation block number",
    );
  }
}

/**
 * Build production network routes for `FakeHost` (the permission/consent host).
 * Reads execute at one captured finalized hash. Submissions are constructed by a
 * generated typed route and resolve only on a typed finalized status.
 */
export function createTypedNetworkHostRoutes<
  Client extends TypedPapiClient,
  Signer extends TypedChainSigner,
>(options: TypedNetworkHostOptions<Client, Signer>): Pick<HostDependencies, "finalizedRead" | "submitAndFinalize"> {
  assertBindingShape(options.binding);

  const validateRequestBinding = (signed: SignedRequest): void => {
    equalBinding(signed.request.network, options.binding);
  };

  const finalizedRead: HostRoute = async (signed, signal): Promise<HostTransportResult> => {
    try {
      validateRequestBinding(signed);
      const route = routeFor(options.routes, signed.request);
      if (route.finality !== "finalized")
        throw new ProductSdkError("invalid_input", "submit route used for finalized read");
      const finalized = await finalizedContext(options.client, options.binding, signal);
      const isSponsoredPreparation = signed.request.capability === "transaction"
        && signed.request.method === "prepare_sponsored_intent";
      if (isSponsoredPreparation) assertSponsoredPreparationAnchor(signed.request.payload, finalized);
      const response = await abortable(
        route.query(signed.request.payload, {
          client: options.client,
          at: finalized.hash,
          atNumber: finalized.number,
          signal,
          authorizationSignature: signed.signature,
        }),
        signal,
      );
      assertJsonValue(response);
      if (isSponsoredPreparation) {
        validatePreparedSponsoredIntent(response, signed.request.payload, options.binding);
      }
      return { finalizedHash: finalized.hash, response };
    } catch (error) {
      throw mapFailure(error);
    }
  };

  const submitAndFinalize: HostRoute = async (signed, signal): Promise<HostTransportResult> => {
    let iterator: AsyncIterator<TypedTransactionStatus> | undefined;
    try {
      validateRequestBinding(signed);
      const route = routeFor(options.routes, signed.request);
      if (route.finality !== "submit-and-finalize")
        throw new ProductSdkError("invalid_input", "read route used for submission");
      const isSponsoredSubmit = signed.request.capability === "transaction"
        && signed.request.method === "submit_sponsored_intent";
      let sponsoredEnvelope: JsonObject | undefined;
      if (isSponsoredSubmit) {
        sponsoredEnvelope = ((signed.request.payload.signed_intent as JsonObject).envelope) as JsonObject;
        if (sponsoredEnvelope.participant === options.signer.accountId)
          throw new ProductSdkError("invalid_input", "participant and outer sponsor signer must be distinct");
      }
      const finalized = await finalizedContext(options.client, options.binding, signal);
      const transaction = route.transaction(signed.request.payload, {
        client: options.client,
        at: finalized.hash,
        atNumber: finalized.number,
        authorizationSignature: signed.signature,
      });
      const statuses = transaction.signSubmitAndWatch(options.signer, { signal });
      iterator = statuses[Symbol.asyncIterator]();
      while (true) {
        const next = await abortable(iterator.next(), signal);
        if (next.done)
          throw new ProductSdkError("runtime_rejected", "transaction stream ended before finalization");
        const status = next.value;
        if (status.type === "rejected") throw status.error;
        if (status.type !== "finalized") continue;
        assertHash(status.blockHash, "finalized transaction block hash");
        assertHash(status.extrinsicHash, "finalized extrinsic hash");
        let sponsoredOutcome: JsonObject | undefined;
        if (isSponsoredSubmit) {
          const dispatched = status.events?.find((event) => event.pallet === "MetaTx"
            && event.event === "Dispatched" && event.fields.result === "Ok");
          if (!dispatched)
            throw new ProductSdkError(
              "runtime_rejected",
              "sponsored submission finalized without MetaTx::Dispatched Ok",
            );
          sponsoredOutcome = {
            version: 1,
            intent_id: sponsoredEnvelope!.intent_id,
            participant: sponsoredEnvelope!.participant,
            sponsor: options.signer.accountId,
            dispatched: true,
            meta_tx_event: "Dispatched",
            inner_result: "Ok",
          };
        }
        const observed = await abortable(
          options.client.getRuntimeIdentityAt(status.blockHash, signal),
          signal,
        );
        equalObservedRuntime(observed, options.binding);
        return {
          finalizedHash: status.blockHash,
          extrinsicHash: status.extrinsicHash,
          lifecycle: {
            version: 1,
            intent_id: signed.request.request_id,
            state: "finalized",
            block_hash: status.blockHash,
            extrinsic_hash: status.extrinsicHash,
          },
          ...(sponsoredOutcome ? { response: sponsoredOutcome } : {}),
        };
      }
    } catch (error) {
      throw mapFailure(error);
    } finally {
      try {
        await iterator?.return?.();
      } catch {
        // A transport close error must not replace the typed terminal outcome.
      }
    }
  };

  return { finalizedRead, submitAndFinalize };
}
