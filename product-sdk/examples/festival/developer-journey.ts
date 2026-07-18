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
import type { JsonObject } from "../../packages/core/src/contract.ts";
import { createApp, type OriginAppRuntime } from "@cord-network/origin-sdk";
import {
  bucketName,
  decimalU64,
  driveName,
  driveRequests,
  objectKey,
  providerRequests,
  s3Requests,
  type AgreementId,
  type BucketId,
  type CloudStorageRuntimeAdapter,
  type ContainerId,
  type ContentCommitment,
  type ContentHash,
  type DriveId,
  type ProviderId,
} from "@cord-network/origin-sdk-cloud-storage";
import { COMMONS_NETWORK_BINDING } from "@cord-network/origin-sdk-descriptors";
import type { SdkResult } from "@cord-network/origin-sdk-errors";
import { createFakeHost } from "@cord-network/origin-sdk-host/testing";
import {
  accountId,
  type IdentityGrantV2,
  type IdentityV2Bridge,
  type IdentityV2Call,
  type IdentityV2InvocationOptions,
} from "@cord-network/origin-sdk-identity";
import {
  normalizedLabel,
  subjectId,
  type NameId,
  type NamesRuntimeAdapter,
} from "@cord-network/origin-sdk-names";
import { submitAndFinalize, type PreparedTransaction } from "@cord-network/origin-sdk-tx";

const PRODUCT = { id: "festival.app", name: "Festival" } as const;
const FINALIZED_HASH = `0x${"71".repeat(32)}` as const;
const FINALIZED_BYTES = new Uint8Array(32).fill(0x71);
const INCARNATION = new Uint8Array(32).fill(0x31);
const SUBJECT_BYTES = new Uint8Array(32).fill(0x41);
const DERIVED_KEY = new Uint8Array(32).fill(0x42);
const NAME_ID = `0x${"51".repeat(32)}` as NameId;
const PROVIDER_ID = accountId("5FestivalProvider") as ProviderId;
const DRIVE_ID = `0x${"61".repeat(32)}` as DriveId;
const BUCKET_ID = `0x${"62".repeat(32)}` as BucketId;
const AGREEMENT_ID = `0x${"63".repeat(32)}` as AgreementId;
const CONTENT_HASH = `0x${"64".repeat(32)}` as ContentHash;
const CONTENT_COMMITMENT = `0x${"65".repeat(32)}` as ContentCommitment;
const CONTAINER_ID = `0x${"66".repeat(32)}` as ContainerId;

interface DeveloperState {
  providerAvailable: boolean;
  objectVersion: string;
  reads: string[];
  writes: string[];
  chainSigners: string[];
  identityOperations: IdentityV2Call[];
}

type Outcome = { readonly code: string; readonly retryable: boolean; readonly detail?: string };

function bytesHex(value: Uint8Array): `0x${string}` {
  return `0x${Buffer.from(value).toString("hex")}`;
}

function outcome<T>(result: SdkResult<T>, detail?: (value: T) => string): Outcome {
  if ("error" in result) return { code: result.error.code, retryable: result.error.retryable };
  return {
    code: "success",
    retryable: false,
    ...(detail === undefined ? {} : { detail: detail(result.value) }),
  };
}

function options(byte: number, freshConsent = false): IdentityV2InvocationOptions {
  return {
    requestId: new Uint8Array(16).fill(byte),
    deadlineBlock: 140n,
    finalizedBlock: 100n,
    finalizedHash: FINALIZED_BYTES,
    currentRecoveryIncarnation: INCARNATION,
    ...(freshConsent ? { operationId: new Uint8Array(16).fill(byte + 0x20) } : {}),
  };
}

function grant<Operation extends IdentityV2Call>(
  operation: Operation,
  byte: number,
  overrides: Partial<IdentityGrantV2<Operation>> = {},
): IdentityGrantV2<Operation> {
  return {
    version: 2,
    id: new Uint8Array(32).fill(byte),
    productId: PRODUCT.id,
    scope: operation,
    recoveryIncarnation: INCARNATION,
    expiresAt: 200n,
    ...overrides,
  };
}

function identityBridge(state: DeveloperState): IdentityV2Bridge {
  return {
    async request(invocation) {
      state.identityOperations.push(invocation.operation);
      switch (invocation.operation) {
        case "identity.subject.derive":
          return { success: true, value: {
            subject: SUBJECT_BYTES,
            derivedPublicKey: DERIVED_KEY,
            epoch: 0,
            recoveryIncarnationHash: new Uint8Array(32).fill(0x43),
            continuity: true,
          } };
        case "identity.entitlements.read":
          return { success: true, value: {
            allowed: true,
            scope: "festival.entry",
            policyVersion: 3,
            expiresAt: 130n,
            freshUntil: 110n,
            finalized: { blockNumber: 100n, blockHash: FINALIZED_BYTES },
          } };
        case "transaction.sign":
          return { success: true, value: {
            transactionHash: new Uint8Array(32).fill(0x44),
            finalized: { blockNumber: 100n, blockHash: FINALIZED_BYTES },
          } };
        default:
          return { success: false, error: {
            code: 109,
            name: "GRANT_REQUIRED",
            retryable: false,
            details: { message: `Festival journey did not grant ${invocation.operation}` },
          } };
      }
    },
  };
}

function prepared(
  state: DeveloperState,
  service: string,
  method: string,
  payload: Readonly<Record<string, unknown>>,
): PreparedTransaction {
  return {
    async *signSubmitAndWatch(signer) {
      const account = (signer as { readonly account?: { readonly address?: string } }).account?.address
        ?? "unknown";
      const signed = await signer.sign({
        account,
        payload: new TextEncoder().encode(`${service}.${method}`),
        purpose: `Festival ${service}.${method}`,
      });
      if ("error" in signed) {
        yield { type: "rejected", code: signed.error.code, message: signed.error.message };
        return;
      }
      state.chainSigners.push(account);
      if (service === "s3" && method === "put_object"
          && payload.expected_object_version !== state.objectVersion) {
        yield { type: "rejected", code: "version_conflict", message: "object version changed" };
        return;
      }
      state.writes.push(`${service}.${method}`);
      if (service === "s3" && method === "put_object") state.objectVersion = "2";
      yield { type: "broadcast" };
      yield {
        type: "finalized",
        blockHash: FINALIZED_HASH,
        transactionHash: `0x${"81".repeat(32)}`,
      };
    },
  };
}

function developerRuntime(state: DeveloperState): OriginAppRuntime {
  const storage: CloudStorageRuntimeAdapter = {
    async read<T>(_at: `0x${string}`, service: "provider" | "drive" | "s3", method: string) {
      state.reads.push(`${service}.${method}`);
      if (service === "provider" && !state.providerAvailable) {
        throw new Error("selected provider is temporarily unavailable");
      }
      if (method === "provider_by_id") return { provider: PROVIDER_ID, status: "active" } as T;
      if (method === "drive_by_id") return { drive_id: DRIVE_ID, version: "1", status: "active" } as T;
      if (method === "bucket_by_id") return { bucket: BUCKET_ID, version: "1", status: "active" } as T;
      if (method === "object_by_key") return {
        bucket: BUCKET_ID,
        key: "passes/today.json",
        version: state.objectVersion,
        content_hash: CONTENT_HASH,
      } as T;
      throw new Error(`unsupported Festival storage read ${service}.${method}`);
    },
    async prepare(_at, service, method, payload) {
      return prepared(state, service, method, payload);
    },
  };
  const names = {
    async rootNameByNormalizedLabel() { return { version: 1, value: NAME_ID }; },
    async resolveSubject() { return { version: 1, value: subjectId(bytesHex(SUBJECT_BYTES)) }; },
    async setSubject(_at: string, _name: NameId, subject: string) {
      return prepared(state, "names", "set_subject", { subject });
    },
  } as unknown as NamesRuntimeAdapter;
  return { attestation: {}, assets: {}, names, storage } as OriginAppRuntime;
}

export async function runFestivalDeveloperJourney(): Promise<JsonObject> {
  const state: DeveloperState = {
    providerAvailable: true,
    objectVersion: "1",
    reads: [],
    writes: [],
    chainSigners: [],
    identityOperations: [],
  };
  const host = createFakeHost({
    accounts: [{ address: "5FestivalDeveloper" }],
    finalizedBlock: { hash: FINALIZED_HASH, number: 100n },
    runtimeIdentity: {
      genesis_hash: COMMONS_NETWORK_BINDING.genesis_hash,
      spec_version: COMMONS_NETWORK_BINDING.spec_version,
      transaction_version: COMMONS_NETWORK_BINDING.transaction_version,
      metadata_hash: COMMONS_NETWORK_BINDING.metadata_hash,
      descriptor_contract_sha256: COMMONS_NETWORK_BINDING.descriptor_contract_sha256,
      chain_spec_source_sha256: COMMONS_NETWORK_BINDING.chain_spec_source_sha256,
    },
  });
  for (const capability of ["chain", "accounts", "signing", "local-storage"] as const) {
    host.grant(PRODUCT.id, capability);
  }
  const created = await createApp({
    product: PRODUCT,
    bridge: host.bridge,
    identityBridge: identityBridge(state),
    runtime: developerRuntime(state),
  });
  assert.equal(created.success, true);
  if ("error" in created) throw created.error;
  const app = created.value;
  const results: Record<string, Outcome> = {};

  const subject = await app.identity.subjectDerive(
    grant("identity.subject.derive", 0x11, { audience: "festival.example" }),
    { productId: PRODUCT.id, context: "festival-entry", verifierAudience: "festival.example" },
    options(0x11),
  );
  results.identity_subject = outcome(subject, (value) => bytesHex(value.subject));
  assert.equal(subject.success, true);
  if ("error" in subject) throw subject.error;

  const entitlement = await app.identity.entitlementsRead(
    grant("identity.entitlements.read", 0x12),
    { subject: subject.value.subject, scope: "festival.entry" },
    options(0x12),
  );
  results.identity_entitlement = outcome(entitlement, (value) => String(value.allowed));

  const linked = await app.names.prepareSetSubject(NAME_ID, subjectId(bytesHex(subject.value.subject)));
  results.names_subject_link = linked.success
    ? outcome(await submitAndFinalize(linked.value, app.signer))
    : outcome(linked);
  const resolved = await app.names.resolveSubject(NAME_ID);
  results.names_subject_resolve = outcome(resolved, (value) => String(value.value));

  const snapshot = await app.cloudStorage.readTogether([
    providerRequests.providerById(PROVIDER_ID),
    driveRequests.driveById(DRIVE_ID),
    s3Requests.bucketById(BUCKET_ID),
  ]);
  results.storage_snapshot = outcome(snapshot, (value) => `${value.finalized_hash}:3`);

  const write = async (request: Parameters<typeof app.cloudStorage.prepare>[0]): Promise<Outcome> => {
    const value = await app.cloudStorage.prepare(request);
    return value.success ? outcome(await submitAndFinalize(value.value, app.signer)) : outcome(value);
  };
  results.provider_agreement = await write(providerRequests.proposeAgreement({
    provider: PROVIDER_ID,
    container_ref: CONTAINER_ID,
    content_commitment: CONTENT_COMMITMENT,
    reservation_ref: null,
    bytes: decimalU64(1024),
    expires_at: "200" as never,
  }));
  results.drive_create = await write(driveRequests.createDrive(driveName("festival-media"), CONTENT_HASH));
  results.s3_bucket_create = await write(s3Requests.createBucket(bucketName("festival-media")));

  const signed = await app.signing.signTransaction(
    grant("transaction.sign", 0x13),
    { payloadHash: new Uint8Array(32).fill(0x51), policyHash: new Uint8Array(32).fill(0x52), expiresAt: 130n },
    options(0x13, true),
  );
  results.separate_signing = outcome(signed, (value) => bytesHex(value.transactionHash));

  state.providerAvailable = false;
  results.provider_unavailable = outcome(await app.cloudStorage.read(providerRequests.providerById(PROVIDER_ID)));
  state.providerAvailable = true;
  results.provider_recovered = outcome(await app.cloudStorage.read(providerRequests.providerById(PROVIDER_ID)));

  results.stale_object_write = await write(s3Requests.putObject(
    BUCKET_ID,
    objectKey("passes/today.json"),
    CONTENT_HASH,
    decimalU64(0),
  ));
  const reread = await app.cloudStorage.read<{ readonly version: string }>(
    s3Requests.objectByKey(BUCKET_ID, objectKey("passes/today.json")),
  );
  results.object_version_reread = outcome(reread, (value) => value.version);
  assert.equal(reread.success, true);
  if ("error" in reread) throw reread.error;
  results.object_write_recovered = await write(s3Requests.putObject(
    BUCKET_ID,
    objectKey("passes/today.json"),
    CONTENT_HASH,
    decimalU64(reread.value.version),
  ));

  host.lose();
  results.host_offline = outcome(await app.names.rootNameByNormalizedLabel(normalizedLabel("festival")));
  host.restore();
  results.host_reconnected = outcome(await app.names.rootNameByNormalizedLabel(normalizedLabel("festival")));

  const revoked = await app.identity.entitlementsRead(
    grant("identity.entitlements.read", 0x14, { revoked: true }),
    { subject: subject.value.subject, scope: "festival.entry" },
    options(0x14),
  );
  results.revoked_identity_grant = outcome(revoked);

  const expected: Record<string, string> = {
    identity_subject: "success",
    identity_entitlement: "success",
    names_subject_link: "success",
    names_subject_resolve: "success",
    storage_snapshot: "success",
    provider_agreement: "success",
    drive_create: "success",
    s3_bucket_create: "success",
    separate_signing: "success",
    provider_unavailable: "query_failed",
    provider_recovered: "success",
    stale_object_write: "version_conflict",
    object_version_reread: "success",
    object_write_recovered: "success",
    host_offline: "host_unavailable",
    host_reconnected: "success",
    revoked_identity_grant: "GRANT_REVOKED",
  };
  for (const [step, code] of Object.entries(expected)) assert.equal(results[step]?.code, code, step);
  assert.equal(results.provider_unavailable?.retryable, true);
  assert.equal(results.host_offline?.retryable, true);
  assert.deepEqual(state.identityOperations, [
    "identity.subject.derive",
    "identity.entitlements.read",
    "transaction.sign",
  ]);
  assert.equal(new Set(state.chainSigners).size, 1);
  assert.ok(state.writes.includes("provider.propose_agreement"));
  assert.ok(state.writes.includes("drive.create_drive"));
  assert.ok(state.writes.includes("s3.create_bucket"));
  assert.ok(state.writes.includes("s3.put_object"));
  await app.close();

  return {
    status: "PASS",
    product_surface: "createApp",
    results,
    pinned_storage_snapshot: snapshot.success ? {
      finalized_hash: snapshot.value.finalized_hash,
      finalized_number: snapshot.value.finalized_number.toString(),
      value_count: snapshot.value.values.length,
    } : null,
    identity_operations: state.identityOperations,
    separate_signing: true,
    native_services: ["Identity", "Names", "StorageProvider", "Drive", "S3", "TransactionSigning"],
    recovery_policy: {
      provider_unavailable: "retry only when retryable, preserving typed request",
      version_conflict: "re-read finalized object version, rebuild request, request signing again",
      host_offline: "restore host transport and use a fresh finalized snapshot",
      revoked_grant: "request a new independently scoped Identity grant; never reuse the revoked grant",
    },
    no_parallel_authority: true,
  } as unknown as JsonObject;
}
