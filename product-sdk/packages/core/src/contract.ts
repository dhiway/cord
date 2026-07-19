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

import { ORBIS_NETWORK_BINDING } from "../../descriptors/generated/orbis-network-binding.ts";

export type JsonValue = null | boolean | number | string | JsonValue[] | { [key: string]: JsonValue };
export type JsonObject = { [key: string]: JsonValue };

export const ERROR_CODES = ["permission_denied", "permission_revoked", "consent_expired", "replay", "cancelled", "timeout", "unsupported_runtime", "metadata_mismatch", "descriptor_mismatch", "inconsistent_snapshot", "invalid_input", "not_authorized", "not_found", "expired", "conflict", "capacity_exceeded", "proof_invalid", "content_unavailable", "content_integrity", "unsupported_surface", "runtime_rejected"] as const;
export type ErrorCode = typeof ERROR_CODES[number];

export class ProductSdkError extends Error {
  readonly version = 1;
  readonly code: ErrorCode;
  readonly retryable: boolean;
  readonly details: JsonObject;
  constructor(code: ErrorCode, message: string, retryable = false, details: JsonObject = {}) {
    super(message); this.name = "ProductSdkError"; this.code = code; this.retryable = retryable; this.details = details;
  }
  toJSON(): JsonObject { return { version: 1, code: this.code, message: this.message, retryable: this.retryable, details: this.details }; }
}

const forbidden = new Set(["scale", "rawscale", "scalebytes", "abi", "contractabi", "contractaddress", "contractaddr", "deploymentaddress", "revivecontract"]);
const normalizeKey = (key: string) => key.normalize("NFKC").toLowerCase().replace(/[^a-z0-9]/g, "");
export function assertNoContractSurface(value: JsonValue, path = "payload"): void {
  if (typeof value === "string" && /(?:raw[\s_-]*scale|contract[\s_-]*(?:abi|address)|revive[\s_-]*contract)/i.test(value)) throw new ProductSdkError("unsupported_surface", `contract-era value at ${path}`);
  if (Array.isArray(value)) return value.forEach((item, index) => assertNoContractSurface(item, `${path}[${index}]`));
  if (value && typeof value === "object") for (const [key, child] of Object.entries(value)) {
    const normalized=normalizeKey(key);
    if (forbidden.has(normalized)||normalized.includes("scale")
      ||normalized==="abi"||normalized.startsWith("abi")||normalized.endsWith("abi")
      ||normalized.includes("contractabi")
      ||(normalized.includes("contract")&&(normalized.includes("address")||normalized.includes("addr")||normalized.includes("deployment")))) throw new ProductSdkError("unsupported_surface", `forbidden product field ${path}.${key}`);
    assertNoContractSurface(child, `${path}.${key}`);
  }
}

export type MethodFinality = "finalized" | "submit-and-finalize";

type Rule =
  | { kind: "string"; min?: number; max?: number; maxBytes?: number; pattern?: RegExp; maxDecimal?: bigint }
  | { kind: "number"; integer?: boolean; min?: number; max?: number }
  | { kind: "boolean" }
  | { kind: "null" }
  | { kind: "literal"; value: string }
  | { kind: "nullable"; item: Rule }
  | { kind: "array"; item: Rule; min?: number; max?: number; unique?: boolean }
  | { kind: "object"; fields: Record<string, Rule> }
  | { kind: "oneOf"; choices: Rule[] }
  | { kind: "nativeWriteTarget" };
type MethodContract = { finality: MethodFinality; fields: Record<string, Rule> };

const string = (options: Omit<Extract<Rule, { kind: "string" }>, "kind"> = {}): Rule => ({ kind: "string", ...options });
const integer = (min = 0, max = Number.MAX_SAFE_INTEGER): Rule => ({ kind: "number", integer: true, min, max });
const boolean: Rule = { kind: "boolean" };
const nil: Rule = { kind: "null" };
const nullable = (item: Rule): Rule => ({ kind: "nullable", item });
const array = (item: Rule, min = 0, max = Number.MAX_SAFE_INTEGER, unique = false): Rule => ({ kind: "array", item, min, max, unique });
const object = (fields: Record<string, Rule>): Rule => ({ kind: "object", fields });
const oneOf = (...choices: Rule[]): Rule => ({ kind: "oneOf", choices });
const literal = (value: string): Rule => ({ kind: "literal", value });

const hash32 = string({ pattern: /^0x[0-9a-f]{64}$/i });
const signature64 = string({ pattern: /^0x[0-9a-f]{128}$/i });
const account = string({ min: 1, max: 128 });
const decimalU64 = string({ pattern: /^(0|[1-9][0-9]{0,19})$/ });
const decimalU32 = string({ pattern: /^(0|[1-9][0-9]{0,9})$/, maxDecimal: 0xffff_ffffn });
const sponsoredBlockNumber = string({
  pattern: /^(0|[1-9][0-9]{0,9})$/,
  maxDecimal: 0xffff_ffffn,
});
const blockNumber = decimalU64;
const u32 = integer(0, 0xffff_ffff);
const pageFields = { cursor: nullable(u32), limit: integer(0, 100) };
const label = string({ min: 1, maxBytes: 63, pattern: /^[a-z0-9](?:[a-z0-9-]*[a-z0-9])?$/ });
const subjectId = string({ min: 38, maxBytes: 64, pattern: /^[1-9A-HJ-NP-Za-km-z]+$/ });
const identityData = oneOf(
  object({ kind: literal("none") }),
  object({ kind: literal("raw"), value: string({ min: 1, maxBytes: 32 }) }),
  object({ kind: literal("blake2_256"), hash: hash32 }),
  object({ kind: literal("sha2_256"), hash: hash32 }),
  object({ kind: literal("keccak_256"), hash: hash32 }),
  object({ kind: literal("sha3_256"), hash: hash32 }),
);
const identityInfo = object({
  display: identityData,
  legal: identityData,
  web: identityData,
  email: identityData,
  image: identityData,
  additional: array(object({ key: identityData, value: identityData }), 0, 32),
});
const identityJudgement = oneOf(
  literal("reasonable"), literal("known_good"), literal("out_of_date"),
  literal("low_quality"), literal("erroneous"),
);
const nativeWriteTarget: Rule = { kind: "nativeWriteTarget" };

const attestationInput = {
  schema: hash32,
  subject_commitment: hash32,
  payload_commitment: hash32,
  status_commitment: hash32,
  parent: nullable(hash32),
  expiry: nullable(blockNumber),
  uniqueness_commitment: nullable(hash32),
  revocable: boolean,
};
const delegatedIntent = object({
  genesis_hash: hash32,
  spec_version: u32,
  action: literal("issue"),
  issuer: account,
  delegate: account,
  ...attestationInput,
  nonce: decimalU64,
  deadline: blockNumber,
});
const delegatedRevokeIntent = object({
  genesis_hash: hash32,
  spec_version: u32,
  action: literal("revoke"),
  revoker: account,
  delegate: account,
  attestation: hash32,
  nonce: decimalU64,
  deadline: blockNumber,
});
const issuerSignature = string({ min: 1, max: 1024 });
const signedDelegatedIssue = object({ intent: delegatedIntent, signature: issuerSignature });
const signedDelegatedRevoke = object({ intent: delegatedRevokeIntent, signature: issuerSignature });
const transactionRef = oneOf(
  object({ kind: literal("position"), block: blockNumber, index: u32 }),
  object({ kind: literal("content_hash"), content_hash: hash32 }),
);
const cidConfig = object({
  codec: decimalU64,
  hashing: oneOf(literal("blake2b256"), literal("sha2_256"), literal("keccak256")),
});

const methods: Record<string, MethodContract> = {};
const add = (capability: string, method: string, finality: MethodFinality, fields: Record<string, Rule>): void => {
  methods[`${capability}.${method}`] = { finality, fields };
};
const read = (capability: string, method: string, fields: Record<string, Rule>): void => add(capability, method, "finalized", fields);
const write = (capability: string, method: string, fields: Record<string, Rule>): void => add(capability, method, "submit-and-finalize", fields);

// Native attestation runtime API and pallet calls.
read("attestation", "schema_by_id", { schema: hash32 });
read("attestation", "attestation_by_id", { attestation: hash32 });
read("attestation", "attestation_live_status", { attestation: hash32 });
read("attestation", "creator_schemas", { creator: account, ...pageFields });
read("attestation", "issuer_attestations", { issuer: account, ...pageFields });
read("attestation", "subject_schema_attestations", {
  subject_commitment: hash32,
  schema: hash32,
  ...pageFields,
});
read("attestation", "next_delegated_nonce", { issuer: account });
read("attestation", "schema_count", {});
read("attestation", "attestation_count", {});
read("attestation", "next_issuance_nonce", { issuer: account });
read("attestation", "external_status", { issuer: account, status_commitment: hash32 });
write("attestation", "create_schema", {
  definition: string({ min: 1, maxBytes: 16 * 1024 }),
  authorized_issuers: array(account, 0, 64, true),
  revocable: boolean,
  unique: boolean,
  index_policy: oneOf(
    literal("none"),
    literal("issuer"),
    literal("subject_and_schema"),
    literal("issuer_and_subject_schema"),
  ),
});
write("attestation", "set_schema_status", {
  schema: hash32,
  status: oneOf(literal("active"), literal("paused"), literal("retired")),
});
write("attestation", "issue", attestationInput);
write("attestation", "issue_delegated", { intent: delegatedIntent, signature: issuerSignature });
write("attestation", "issue_batch", { items: array(object(attestationInput), 1, 64) });
write("attestation", "revoke", { attestation: hash32 });
write("attestation", "set_emergency_pause", { paused: boolean });
write("attestation", "force_schema_status", {
  schema: hash32,
  status: oneOf(literal("active"), literal("paused"), literal("retired")),
});
write("attestation", "force_revoke", { attestation: hash32 });
write("attestation", "revoke_delegated", {
  intent: delegatedRevokeIntent,
  signature: issuerSignature,
});
write("attestation", "issue_delegated_batch", {
  items: array(signedDelegatedIssue, 1, 64),
});
write("attestation", "revoke_batch", {
  attestations: array(hash32, 1, 64, true),
});
write("attestation", "revoke_delegated_batch", {
	items: array(signedDelegatedRevoke, 1, 64),
});
write("attestation", "revoke_external_status", { status_commitment: hash32 });
write("attestation", "revoke_external_status_batch", {
	status_commitments: array(hash32, 1, 64, true),
});

// Native People/People-Lite runtime API and pallet calls.
read("identity", "identity_status", { account });
read("identity", "personhood_status", { account });
read("identity", "attestation_allowance", { account });
write("identity", "set_identity", { info: identityInfo });
write("identity", "clear_identity", {});
write("identity", "request_judgement", { registrar: account });
write("identity", "cancel_judgement_request", { registrar: account });
write("identity", "provide_judgement", {
  target: account,
  judgement: identityJudgement,
  identity_hash: hash32,
});
write("identity", "attest_lite_person", {
  candidate: account,
  candidate_signature: oneOf(
    object({ scheme: literal("sr25519"), bytes: string({ pattern: /^0x[0-9a-f]{128}$/ }) }),
    object({ scheme: literal("ed25519"), bytes: string({ pattern: /^0x[0-9a-f]{128}$/ }) }),
    object({ scheme: literal("ecdsa"), bytes: string({ pattern: /^0x[0-9a-f]{130}$/ }) }),
  ),
  ring_vrf_key: hash32,
  proof_of_ownership: string({ pattern: /^0x[0-9a-f]{128}$/ }),
});

// Native Orbis Names runtime API and pallet calls.
read("names", "label_policy_version", {});
read("names", "name_by_id", { name: hash32 });
read("names", "root_name_by_normalized_label", { label });
read("names", "owner_names", { owner: account, ...pageFields });
read("names", "controllers", { name: hash32 });
for (const method of ["resolve_address", "resolve_subject", "resolve_attestation", "resolve_content", "name_status"])
  read("names", method, { name: hash32 });
read("names", "resolve_text", { name: hash32, key: string({ min: 1, maxBytes: 32 }) });
read("names", "primary_name", { owner: account });
write("names", "commit", { commitment: hash32 });
write("names", "cancel_commitment", { commitment: hash32 });
write("names", "prune_expired_commitment", { owner: account, commitment: hash32 });
write("names", "register", { parent: nullable(hash32), label, salt: string({ min: 1, maxBytes: 64 }) });
write("names", "renew", { name: hash32, additional_period: blockNumber });
write("names", "transfer", { name: hash32, new_owner: account });
write("names", "add_controller", { name: hash32, controller: account });
write("names", "remove_controller", { name: hash32, controller: account });
write("names", "set_address", { name: hash32, address: nullable(string({ min: 1, maxBytes: 128 })) });
write("names", "set_subject", { name: hash32, subject: nullable(subjectId) });
write("names", "set_attestation", { name: hash32, attestation: nullable(hash32) });
write("names", "set_content", { name: hash32, content: nullable(hash32) });
write("names", "set_text", {
  name: hash32,
  key: string({ min: 1, maxBytes: 32 }),
  value: nullable(string({ min: 1, maxBytes: 256 })),
});
write("names", "set_primary_name", { name: nullable(hash32) });
write("names", "release", { name: hash32 });
write("names", "remove_expired_name", { name: hash32 });
write("names", "reserve_name", {
  parent: nullable(hash32),
  label,
  beneficiary: nullable(account),
  expires_at: nullable(blockNumber),
});
write("names", "clear_reservation", { name: hash32 });
write("names", "set_label_protection", { label, protected: boolean });
write("names", "set_paused", { paused: boolean });
write("names", "force_transfer", { name: hash32, new_owner: account });
write("names", "force_revoke", { name: hash32 });
write("names", "set_registrar", { registrar: account, enabled: boolean });

// Orbis Storage storage runtime API and native calls.
read("storage", "account_authorization", { account });
read("storage", "can_store", { account, data_len: u32 });
read("storage", "can_renew", { account, entry: transactionRef });
read("storage", "stored_content_provenance", {
  reference: object({ block: blockNumber, transaction_index: u32 }),
});
read("storage", "resource_reservation", { reservation_id: decimalU64 });
read("storage", "resource_reservation_link", { reservation_id: decimalU64, content_hash: hash32 });
read("storage", "resource_provider_ref", { reservation_id: decimalU64 });
const contentBase64 = string({ min: 4, pattern: /^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/ });
write("storage", "store", { content_base64: contentBase64 });
write("storage", "store_with_cid_config", { cid_config: cidConfig, content_base64: contentBase64 });
write("storage", "store_reserved", {
  reservation_id: decimalU64,
  cid_config: cidConfig,
  content_base64: contentBase64,
});
write("storage", "renew_reserved", { reservation_id: decimalU64, content_hash: hash32 });
write("storage", "attach_provider", { reservation_id: decimalU64, provider_ref: hash32 });
write("storage", "renew", { entry: transactionRef });
write("storage", "force_renew", { entry: transactionRef });
write("storage", "enable_auto_renew", { content_hash: hash32 });
write("storage", "disable_auto_renew", { content_hash: hash32 });

// Storage-provider finalized views and pallet calls.
read("storage", "provider_by_id", { provider: account });
read("storage", "providers", pageFields);
read("storage", "agreement_by_id", { agreement_id: hash32 });
read("storage", "provider_agreements", { provider: account, ...pageFields });
read("storage", "agreement_nonce", { owner: account });
read("storage", "challenge_by_id", { challenge_id: hash32 });
read("storage", "challenges_at", { block: blockNumber, ...pageFields });
read("storage", "can_accept_capacity", { provider: account, additional_bytes: decimalU64 });
read("storage", "bucket_checkpoint", { bucket: hash32 });
const providerFields = {
  provider: account,
  endpoint: string({ min: 1, maxBytes: 512 }),
  service_key: string({ min: 1, maxBytes: 128 }),
  capacity_bytes: decimalU64,
};
write("storage", "register_provider", providerFields);
write("storage", "update_provider", providerFields);
write("storage", "set_provider_status", {
  provider: account,
  status: oneOf(literal("active"), literal("suspended")),
});
write("storage", "remove_provider", { provider: account });
write("storage", "heartbeat", {});
write("storage", "propose_agreement", {
  provider: account,
  container_ref: hash32,
  content_commitment: hash32,
  reservation_ref: nullable(decimalU64),
  bytes: decimalU64,
  expires_at: blockNumber,
});
for (const method of ["accept_agreement", "cancel_agreement", "accept_renewal", "expire_agreement", "prune_agreement"])
  write("storage", method, { agreement_id: hash32 });
write("storage", "issue_challenge", {
  agreement_id: hash32,
  expected_commitment: hash32,
  due_at: blockNumber,
});
write("storage", "timeout_challenge", { challenge_id: hash32 });
write("storage", "request_renewal", { agreement_id: hash32, expires_at: blockNumber });
write("storage", "acknowledge_manifest_deletion", {
  manifest: hash32,
  evidence_hash: hash32,
  service_key: hash32,
  signature: signature64,
});

// Drive finalized views and pallet calls.
read("storage", "drive_by_id", { drive_id: hash32 });
read("storage", "owner_drives", { owner: account, ...pageFields });
read("storage", "drive_controllers", { drive_id: hash32, ...pageFields });
read("storage", "next_drive_nonce", { owner: account });
write("storage", "create_drive", {
  name: string({ min: 1, maxBytes: 128 }),
  root_storage_ref: nullable(hash32),
});
write("storage", "update_root", {
  drive_id: hash32,
  expected_version: decimalU64,
  root_storage_ref: nullable(hash32),
});
write("storage", "drive.set_controller", { drive_id: hash32, controller: account, enabled: boolean });
write("storage", "transfer_drive", { drive_id: hash32, new_owner: account });
write("storage", "archive_drive", { drive_id: hash32 });

// S3 finalized views and pallet calls.
const bucketName = string({
  min: 1,
  maxBytes: 63,
  pattern: /^(?!.*(?:\.\.|\.-|-\.))[a-z0-9](?:[a-z0-9.-]*[a-z0-9])?$/,
});
const objectKey = string({ min: 1, maxBytes: 1024 });
read("storage", "bucket_by_id", { bucket: hash32 });
read("storage", "bucket_by_name", { name: bucketName });
read("storage", "owner_buckets", { owner: account, ...pageFields });
read("storage", "bucket_object_keys", {
  bucket: hash32,
  prefix: nullable(string({ min: 0, maxBytes: 1024 })),
  cursor: nullable(object({ snapshot_version: decimalU64, last_key: objectKey })),
  limit: integer(1, 100),
});
read("storage", "object_by_key", { bucket: hash32, key: objectKey });
read("storage", "object_history", { bucket: hash32, key: objectKey, ...pageFields });
read("storage", "object_id", { bucket: hash32, key: objectKey });
write("storage", "create_bucket", { name: bucketName });
write("storage", "s3.set_controller", {
  bucket: hash32,
  expected_bucket_version: decimalU64,
  controller: account,
  enabled: boolean,
});
write("storage", "transfer_bucket", {
  bucket: hash32,
  expected_bucket_version: decimalU64,
  new_owner: account,
});
write("storage", "set_archived", { bucket: hash32, expected_bucket_version: decimalU64, archived: boolean });
write("storage", "set_versioning", { bucket: hash32, expected_bucket_version: decimalU64, enabled: boolean });
write("storage", "put_object", {
  bucket: hash32,
  key: objectKey,
  content_hash: hash32,
  expected_object_version: nullable(decimalU64),
});
write("storage", "delete_object", {
  bucket: hash32,
  key: objectKey,
  expected_object_version: decimalU64,
});
write("storage", "delete_bucket", { bucket: hash32, expected_bucket_version: decimalU64 });

// Typed sponsored transaction host contract. The v8 transport owns all SCALE construction.
const sponsoredMortality = object({
  valid_from: sponsoredBlockNumber,
  valid_until: sponsoredBlockNumber,
});
const sponsoredEnvelopeFields = {
  version: integer(1, 1),
  signing_domain: literal("orbis/meta-intent/v7"),
  genesis_hash: hash32,
  spec_version: u32,
  transaction_version: u32,
  metadata_hash: hash32,
  participant: account,
  nonce: decimalU32,
  mortality: sponsoredMortality,
  target: nativeWriteTarget,
  signing_payload_hash: hash32,
  intent_id: hash32,
};
read("transaction", "prepare_sponsored_intent", {
  participant: account,
  nonce: decimalU32,
  mortality: sponsoredMortality,
  target: nativeWriteTarget,
});
write("transaction", "submit_sponsored_intent", {
  signed_intent: object({
    envelope: object(sponsoredEnvelopeFields),
    participant_signature: oneOf(
      object({
        scheme: literal("sr25519"),
        value: string({ pattern: /^0x[0-9a-f]{128}$/ }),
      }),
      object({
        scheme: literal("ed25519"),
        value: string({ pattern: /^0x[0-9a-f]{128}$/ }),
      }),
      object({
        scheme: literal("ecdsa"),
        value: string({ pattern: /^0x[0-9a-f]{130}$/ }),
      }),
    ),
  }),
});

function invalid(path: string): never {
  throw new ProductSdkError("invalid_input", `invalid ${path}`);
}

function validateRule(value: JsonValue | undefined, rule: Rule, path: string): void {
  switch (rule.kind) {
    case "string": {
      if (typeof value !== "string") invalid(path);
      const bytes = new TextEncoder().encode(value).length;
      if ((rule.min !== undefined && value.length < rule.min) ||
          (rule.max !== undefined && value.length > rule.max) ||
          (rule.maxBytes !== undefined && bytes > rule.maxBytes) ||
          (rule.pattern && !rule.pattern.test(value)) ||
          (rule.maxDecimal !== undefined && BigInt(value) > rule.maxDecimal)) invalid(path);
      return;
    }
    case "number":
      if (typeof value !== "number" || !Number.isFinite(value) ||
          (rule.integer && !Number.isInteger(value)) ||
          (rule.min !== undefined && value < rule.min) ||
          (rule.max !== undefined && value > rule.max)) invalid(path);
      return;
    case "boolean": if (typeof value !== "boolean") invalid(path); return;
    case "null": if (value !== null) invalid(path); return;
    case "literal": if (value !== rule.value) invalid(path); return;
    case "nullable": if (value !== null) validateRule(value, rule.item, path); return;
    case "array": {
      if (!Array.isArray(value) ||
          (rule.min !== undefined && value.length < rule.min) ||
          (rule.max !== undefined && value.length > rule.max)) invalid(path);
      value.forEach((item, index) => validateRule(item, rule.item, `${path}[${index}]`));
      if (rule.unique && new Set(value.map((item) => JSON.stringify(item))).size !== value.length) invalid(path);
      return;
    }
    case "object": {
      if (!value || typeof value !== "object" || Array.isArray(value)) invalid(path);
      const keys = Object.keys(value);
      const expected = Object.keys(rule.fields);
      if (keys.length !== expected.length || keys.some((key) => !(key in rule.fields))) invalid(path);
      for (const [name, child] of Object.entries(rule.fields))
        validateRule((value as JsonObject)[name], child, `${path}.${name}`);
      return;
    }
    case "oneOf": {
      let matches = 0;
      for (const choice of rule.choices) {
        try { validateRule(value, choice, path); matches++; } catch (error) {
          if (!(error instanceof ProductSdkError) || error.code !== "invalid_input") throw error;
        }
      }
      if (matches !== 1) invalid(path);
      return;
    }
    case "nativeWriteTarget": {
      if (!value || typeof value !== "object" || Array.isArray(value)) invalid(path);
      const target = value as JsonObject;
      if (Object.keys(target).sort().join() !== ["capability", "method", "payload"].sort().join()
        || typeof target.capability !== "string" || typeof target.method !== "string"
        || !target.payload || typeof target.payload !== "object" || Array.isArray(target.payload)) invalid(path);
      const contract = methods[`${target.capability}.${target.method}`];
      if (!contract || contract.finality !== "submit-and-finalize" || target.capability === "transaction") invalid(path);
      validateRule(target.payload as JsonObject, object(contract.fields), `${path}.payload`);
      return;
    }
  }
}

export function assertMethodPayload(capability: string, method: string, payload: JsonObject): void {
  assertNoContractSurface(payload);
  const contract = methods[`${capability}.${method}`];
  if (!contract) throw new ProductSdkError("unsupported_surface", `unsupported product method ${capability}.${method}`);
  validateRule(payload, object(contract.fields), "payload");
  if (capability === "transaction") {
    const input = method === "submit_sponsored_intent"
      ? (payload.signed_intent as JsonObject).envelope as JsonObject
      : payload;
    const mortality = input.mortality as JsonObject;
    const validFrom = BigInt(String(mortality.valid_from));
    const validUntil = BigInt(String(mortality.valid_until));
    const period = validUntil - validFrom;
    const quantizeFactor = period > 4_096n ? period >> 12n : 1n;
    if (period < 4n || period > 65_536n || (period & (period - 1n)) !== 0n
      || validFrom % quantizeFactor !== 0n)
      invalid("payload.mortality.valid_until");
    if (method === "submit_sponsored_intent") {
      if (input.genesis_hash !== ORBIS_NETWORK_BINDING.genesis_hash
        || input.spec_version !== ORBIS_NETWORK_BINDING.spec_version
        || input.transaction_version !== ORBIS_NETWORK_BINDING.transaction_version)
        throw new ProductSdkError("unsupported_runtime", "sponsored intent runtime identity mismatch");
      if (input.metadata_hash !== ORBIS_NETWORK_BINDING.metadata_hash)
        throw new ProductSdkError("metadata_mismatch", "sponsored intent metadata hash mismatch");
    }
  }
}

function sampleRule(rule: Rule): JsonValue {
  switch (rule.kind) {
    case "string": {
      if (rule === contentBase64) return "AQID";
      const candidates = [
        `0x${"11".repeat(32)}`,
        `0x${"11".repeat(64)}`,
        `0x${"11".repeat(65)}`,
        "1", "AQID", "active", "sample", "a",
      ];
      for (const candidate of candidates) {
        try { validateRule(candidate, rule, "sample"); return candidate; } catch {}
      }
      throw new Error("no canonical sample for string rule");
    }
    case "number": return Math.max(rule.min ?? 0, 1);
    case "boolean": return true;
    case "null": return null;
    case "literal": return rule.value;
    case "nullable": return null;
    case "array": return Array.from({ length: rule.min ?? 0 }, () => sampleRule(rule.item));
    case "object": return Object.fromEntries(Object.entries(rule.fields).map(([name, child]) => [name, sampleRule(child)]));
    case "oneOf": return sampleRule(rule.choices[0]);
    case "nativeWriteTarget": return {
      capability: "identity",
      method: "clear_identity",
      payload: {},
    };
  }
}

export function canonicalMethodPayload(capability: string, method: string): JsonObject {
  const contract = methods[`${capability}.${method}`];
  if (!contract) throw new ProductSdkError("unsupported_surface", `unsupported product method ${capability}.${method}`);
  let payload = sampleRule(object(contract.fields)) as JsonObject;
  if (capability === "transaction") {
    const base = {
      participant: "participant-account",
      nonce: "1",
      mortality: { valid_from: "1", valid_until: "65" },
      target: { capability: "identity", method: "clear_identity", payload: {} },
    };
    payload = method === "prepare_sponsored_intent" ? base : {
      signed_intent: {
        envelope: {
          version: 1,
          signing_domain: "orbis/meta-intent/v7",
          genesis_hash: ORBIS_NETWORK_BINDING.genesis_hash,
          spec_version: ORBIS_NETWORK_BINDING.spec_version,
          transaction_version: ORBIS_NETWORK_BINDING.transaction_version,
          metadata_hash: ORBIS_NETWORK_BINDING.metadata_hash,
          ...base,
          signing_payload_hash: `0x${"22".repeat(32)}`,
          intent_id: `0x${"11".repeat(32)}`,
        },
        participant_signature: { scheme: "sr25519", value: `0x${"33".repeat(64)}` },
      },
    };
  }
  validateRule(payload, object(contract.fields), "payload");
  assertMethodPayload(capability, method, payload);
  return payload;
}

function ruleSchema(rule: Rule): JsonObject {
  switch (rule.kind) {
    case "string": return { type: "string", ...(rule.min === undefined ? {} : { minLength: rule.min }), ...(rule.max === undefined ? {} : { maxLength: rule.max }), ...(rule.maxBytes === undefined ? {} : { maxUtf8Bytes: rule.maxBytes }), ...(rule.pattern ? { pattern: rule.pattern.source } : {}), ...(rule.maxDecimal === undefined ? {} : { maxDecimal: rule.maxDecimal.toString() }) };
    case "number": return { type: rule.integer ? "integer" : "number", ...(rule.min === undefined ? {} : { minimum: rule.min }), ...(rule.max === undefined ? {} : { maximum: rule.max }) };
    case "boolean": return { type: "boolean" };
    case "null": return { type: "null" };
    case "literal": return { const: rule.value };
    case "nullable": return { anyOf: [{ type: "null" }, ruleSchema(rule.item)] };
    case "array": return { type: "array", items: ruleSchema(rule.item), ...(rule.min === undefined ? {} : { minItems: rule.min }), ...(rule.max === undefined ? {} : { maxItems: rule.max }), ...(rule.unique ? { uniqueItems: true } : {}) };
    case "object": return { type: "object", additionalProperties: false, required: Object.keys(rule.fields), properties: Object.fromEntries(Object.entries(rule.fields).map(([name, child]) => [name, ruleSchema(child)])) };
    case "oneOf": return { oneOf: rule.choices.map(ruleSchema) };
    case "nativeWriteTarget": return {
      oneOf: Object.entries(methods)
        .filter(([identity, contract]) => contract.finality === "submit-and-finalize"
          && !identity.startsWith("transaction."))
        .map(([identity, contract]) => {
          const [capability, method] = identity.split(".");
          return {
            type: "object",
            additionalProperties: false,
            required: ["capability", "method", "payload"],
            properties: {
              capability: { const: capability },
              method: { const: method },
              payload: ruleSchema(object(contract.fields)),
            },
          };
        }),
    };
  }
}

export function methodPayloadSchema(capability: string, method: string): JsonObject {
  const contract = methods[`${capability}.${method}`];
  if (!contract) throw new ProductSdkError("unsupported_surface", `unsupported product method ${capability}.${method}`);
  return ruleSchema(object(contract.fields));
}

export function assertMethodFinality(capability: string, method: string, finality: MethodFinality): void {
  const contract = methods[`${capability}.${method}`];
  if (!contract) throw new ProductSdkError("unsupported_surface", `unsupported product method ${capability}.${method}`);
  if (contract.finality !== finality)
    throw new ProductSdkError("invalid_input", `${capability}.${method} requires ${contract.finality}`);
}
export function assertRuntimeIdentity(genesis: string, spec: number, tx: number, metadata: string, descriptor: string, chainSpec: string, activationState: "candidate-pending" | "production-approved", productionActivationReady: boolean, accessMode: "candidate" | "production"): void {
  if (genesis !== ORBIS_NETWORK_BINDING.genesis_hash) throw new ProductSdkError("unsupported_runtime", "unrecognized Orbis genesis identity");
  if (spec !== ORBIS_NETWORK_BINDING.spec_version || tx !== ORBIS_NETWORK_BINDING.transaction_version) throw new ProductSdkError("unsupported_runtime", `unsupported Commons runtime ${spec}/${tx}`);
  if (metadata !== ORBIS_NETWORK_BINDING.metadata_hash) throw new ProductSdkError("metadata_mismatch", "metadata hash mismatch");
  if (chainSpec !== ORBIS_NETWORK_BINDING.chain_spec_source_sha256) throw new ProductSdkError("unsupported_runtime", "chain-spec source mismatch");
  if (descriptor !== ORBIS_NETWORK_BINDING.descriptor_contract_sha256) throw new ProductSdkError("descriptor_mismatch", "descriptor hash mismatch");
  if (activationState !== ORBIS_NETWORK_BINDING.activation_state
    || productionActivationReady !== ORBIS_NETWORK_BINDING.production_activation_ready)
    throw new ProductSdkError("unsupported_runtime", "network activation state does not match the signed SDK freeze");
  if (accessMode === "candidate") {
    if (activationState !== "candidate-pending" || productionActivationReady)
      throw new ProductSdkError("unsupported_runtime", "candidate access is unavailable for the activated production network");
  } else if (activationState !== "production-approved" || !productionActivationReady) {
    throw new ProductSdkError("unsupported_runtime", "production access rejects the unsigned candidate/PENDING network");
  }
}
export function validateLifecycle(value:any):void{
  const states=["draft","authorized","submitted","included","finalized","rejected","expired","cancelled"],hash=/^0x[0-9a-fA-F]{64}$/;
  if(!value||typeof value!=="object"||Array.isArray(value)||!Object.keys(value).every(k=>["version","intent_id","state","block_hash","extrinsic_hash","error"].includes(k))||value.version!==1||typeof value.intent_id!=="string"||value.intent_id.length<16||value.intent_id.length>128||!states.includes(value.state))throw new ProductSdkError("invalid_input","invalid lifecycle envelope");
  if(value.block_hash!==undefined&&(typeof value.block_hash!=="string"||!hash.test(value.block_hash)))throw new ProductSdkError("invalid_input","invalid lifecycle block hash");if(value.extrinsic_hash!==undefined&&(typeof value.extrinsic_hash!=="string"||!hash.test(value.extrinsic_hash)))throw new ProductSdkError("invalid_input","invalid lifecycle extrinsic hash");
  if(value.error!==undefined){const e=value.error;if(!e||typeof e!=="object"||Array.isArray(e)||!Object.keys(e).every(k=>["version","code","message","retryable","details"].includes(k))||e.version!==1||!ERROR_CODES.includes(e.code)||typeof e.message!=="string"||!e.message||e.message.length>512||typeof e.retryable!=="boolean"||(e.details!==undefined&&(!e.details||typeof e.details!=="object"||Array.isArray(e.details))))throw new ProductSdkError("invalid_input","invalid lifecycle error");}
  if(value.state==="included"&&!value.block_hash)throw new ProductSdkError("invalid_input","included requires block hash");if(value.state==="finalized"&&(!value.block_hash||!value.extrinsic_hash||value.error))throw new ProductSdkError("invalid_input","finalized evidence invalid");if(["rejected","expired","cancelled"].includes(value.state)&&!value.error)throw new ProductSdkError("invalid_input","terminal failure requires error");if(value.state==="cancelled"&&value.error.code!=="cancelled")throw new ProductSdkError("invalid_input","cancelled state/error mismatch");if(["draft","authorized"].includes(value.state)&&(value.block_hash||value.extrinsic_hash||value.error))throw new ProductSdkError("invalid_input","pre-submit state has terminal evidence");
}
