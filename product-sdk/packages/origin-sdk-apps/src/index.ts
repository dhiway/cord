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


import type { CommonsChainClient } from "@cord-network/origin-sdk-chain-client";
import {
  parseContentCid,
  rawContentAddress,
  type ContentAddress,
} from "@cord-network/origin-sdk-cloud-storage";
import { OriginSdkError, asSdkError, type SdkResult } from "@cord-network/origin-sdk-errors";
import type {
  HostCapability,
  OriginHostClient,
  ProductIdentity,
} from "@cord-network/origin-sdk-host";
import {
  contentCommitment,
  nameId,
  type AccountId,
  type AttestationId,
  type ContentCommitment,
  type NameId,
  type NamesRuntimeAdapter,
} from "@cord-network/origin-sdk-names";
import { err, ok } from "@cord-network/origin-sdk-result";
import type { PreparedTransaction } from "@cord-network/origin-sdk-tx";

export const ORIGIN_APP_MANIFEST_SCHEMA = "cord.origin.app-manifest" as const;
export const ORIGIN_APP_MANIFEST_VERSION = 1 as const;

export type OriginRequestedCapability = HostCapability
  | "camera"
  | "nfc"
  | "bluetooth"
  | "location"
  | "biometrics"
  | "external-urls";

export interface OriginAppManifestV1 {
  readonly schema: typeof ORIGIN_APP_MANIFEST_SCHEMA;
  readonly schemaVersion: typeof ORIGIN_APP_MANIFEST_VERSION;
  readonly product: ProductIdentity;
  readonly nameId: NameId;
  readonly version: string;
  readonly channel: string;
  readonly entrypoint: string;
  readonly contentFormat: "static" | "pwa";
  readonly bundle: {
    readonly address: ContentAddress;
    readonly size: number;
  };
  readonly requestedCapabilities: readonly OriginRequestedCapability[];
  readonly attestation?: AttestationId;
}

export interface StoredOriginAppManifest {
  readonly manifest: OriginAppManifestV1;
  readonly bytes: Uint8Array;
  readonly commitment: ContentCommitment;
  readonly address: ContentAddress;
}

export interface OriginAppContentStore {
  put(bytes: Uint8Array, contentType: string, signal?: AbortSignal): Promise<{
    readonly commitment: ContentCommitment;
    readonly address: ContentAddress;
  }>;
  get(commitment: ContentCommitment, signal?: AbortSignal): Promise<Uint8Array>;
}

export interface ResolvedOriginApp {
  readonly manifest: OriginAppManifestV1;
  readonly manifestCommitment: ContentCommitment;
  readonly owner: AccountId;
  readonly finalized: { readonly hash: `0x${string}`; readonly number: bigint };
  readonly launch: {
    readonly isolation: "product-origin";
    readonly productId: string;
    readonly entrypoint: string;
    readonly bundle: ContentAddress;
    readonly requestedCapabilities: readonly OriginRequestedCapability[];
  };
}

export interface OriginAppsClient {
  storeManifest(manifest: OriginAppManifestV1, signal?: AbortSignal): Promise<SdkResult<StoredOriginAppManifest>>;
  prepareManifestBinding(stored: StoredOriginAppManifest, signal?: AbortSignal): Promise<SdkResult<PreparedTransaction>>;
  resolveApp(name: NameId, signal?: AbortSignal): Promise<SdkResult<ResolvedOriginApp>>;
}

const ALL_CAPABILITIES: readonly OriginRequestedCapability[] = [
  "accounts", "chain", "signing", "local-storage", "preimages", "resources", "statements",
  "camera", "nfc", "bluetooth", "location", "biometrics", "external-urls",
];
const utf8 = new TextEncoder();
const decoder = new TextDecoder("utf-8", { fatal: true });

function appError<T>(code: string, message: string, retryable = false): SdkResult<T> {
  return err(new OriginSdkError({
    source: "apps",
    domain: "application",
    code,
    message,
    retryable,
  }));
}

function throwSdkResult<T>(result: SdkResult<T>): T {
  if (result.success) return result.value;
  throw new OriginSdkError({
    source: result.error.source,
    domain: result.error.domain,
    code: result.error.code,
    message: result.error.message,
    retryable: result.error.retryable,
    ...(result.error.details === undefined ? {} : { details: result.error.details }),
  });
}

function hashFromAddress(address: ContentAddress): ContentCommitment {
  const parsed = parseContentCid(address.cid);
  return contentCommitment(`0x${Array.from(
    parsed.digest,
    (byte) => byte.toString(16).padStart(2, "0"),
  ).join("")}`);
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function validateString(value: unknown, label: string, maximum: number): string {
  if (typeof value !== "string" || value.length === 0 || utf8.encode(value).length > maximum) {
    throw new TypeError(`${label} must contain 1-${maximum} UTF-8 bytes`);
  }
  return value;
}

function normalizeManifest(value: OriginAppManifestV1): OriginAppManifestV1 {
  if (!isRecord(value) || value.schema !== ORIGIN_APP_MANIFEST_SCHEMA || value.schemaVersion !== 1) {
    throw new TypeError("manifest schema must be cord.origin.app-manifest v1");
  }
  if (!isRecord(value.product)
    || !/^[a-z0-9][a-z0-9._-]{2,63}$/.test(value.product.id)
    || !value.product.name.trim()) throw new TypeError("manifest product identity is invalid");
  nameId(value.nameId);
  if (!/^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(?:-[0-9A-Za-z.-]+)?$/.test(value.version)) {
    throw new TypeError("manifest version must be semantic version syntax");
  }
  if (!/^[a-z0-9][a-z0-9-]{0,31}$/.test(value.channel)) {
    throw new TypeError("manifest channel must be 1-32 lowercase characters");
  }
  const entrypoint = validateString(value.entrypoint, "manifest entrypoint", 256);
  if (entrypoint.startsWith("/") || entrypoint.includes("\\")
    || entrypoint.split("/").some((part) => part === "" || part === "." || part === "..")
    || /[?#]/.test(entrypoint)) throw new TypeError("manifest entrypoint must be a safe relative path");
  if (value.contentFormat !== "static" && value.contentFormat !== "pwa") {
    throw new TypeError("manifest content format is unsupported");
  }
  if (!isRecord(value.bundle) || !isRecord(value.bundle.address)
    || typeof value.bundle.address.cid !== "string"
    || (value.bundle.address.codec !== "raw" && value.bundle.address.codec !== "dag-pb")
    || (value.bundle.address.multihash !== "blake2b-256"
      && value.bundle.address.multihash !== "sha2-256")) {
    throw new TypeError("manifest bundle address is invalid");
  }
  const parsed = parseContentCid(value.bundle.address.cid);
  if (parsed.codec !== value.bundle.address.codec
    || parsed.multihash !== value.bundle.address.multihash) {
    throw new TypeError("manifest bundle declaration does not match its CID");
  }
  if (!Number.isSafeInteger(value.bundle.size) || value.bundle.size < 1) {
    throw new TypeError("manifest bundle size must be a positive safe integer");
  }
  if (!Array.isArray(value.requestedCapabilities)
    || value.requestedCapabilities.some((capability) => !ALL_CAPABILITIES.includes(capability))
    || new Set(value.requestedCapabilities).size !== value.requestedCapabilities.length) {
    throw new TypeError("manifest requested capabilities must be unique supported values");
  }
  if (value.attestation !== undefined) contentCommitment(value.attestation);
  return {
    schema: ORIGIN_APP_MANIFEST_SCHEMA,
    schemaVersion: 1,
    product: { id: value.product.id, name: value.product.name.trim() },
    nameId: nameId(value.nameId),
    version: value.version,
    channel: value.channel,
    entrypoint,
    contentFormat: value.contentFormat,
    bundle: {
      address: {
        cid: value.bundle.address.cid,
        codec: value.bundle.address.codec,
        multihash: value.bundle.address.multihash,
      },
      size: value.bundle.size,
    },
    requestedCapabilities: [...value.requestedCapabilities].sort(),
    ...(value.attestation === undefined ? {} : { attestation: value.attestation }),
  };
}

function canonicalJson(value: unknown): string {
  if (value === null || typeof value === "boolean" || typeof value === "string") {
    return JSON.stringify(value);
  }
  if (typeof value === "number") {
    if (!Number.isFinite(value)) throw new TypeError("manifest numbers must be finite");
    return JSON.stringify(value);
  }
  if (Array.isArray(value)) return `[${value.map(canonicalJson).join(",")}]`;
  if (isRecord(value)) {
    return `{${Object.keys(value).sort().map((key) =>
      `${JSON.stringify(key)}:${canonicalJson(value[key])}`).join(",")}}`;
  }
  throw new TypeError("manifest contains a non-JSON value");
}

export function encodeOriginAppManifest(manifest: OriginAppManifestV1): Uint8Array {
  const bytes = utf8.encode(canonicalJson(normalizeManifest(manifest)));
  if (bytes.length > 64 * 1024) throw new TypeError("manifest exceeds 65536 bytes");
  return bytes;
}

export function decodeOriginAppManifest(bytes: Uint8Array): OriginAppManifestV1 {
  if (!(bytes instanceof Uint8Array) || bytes.length < 1 || bytes.length > 64 * 1024) {
    throw new TypeError("manifest bytes must contain 1-65536 bytes");
  }
  const parsed: unknown = JSON.parse(decoder.decode(bytes));
  return normalizeManifest(parsed as OriginAppManifestV1);
}

export function createHostOriginAppContentStore(host: OriginHostClient): OriginAppContentStore {
  return {
    async put(bytes, contentType, signal) {
      const address = rawContentAddress(bytes, "blake2b-256");
      const expected = hashFromAddress(address);
      const reference = throwSdkResult(await host.putPreimage(bytes, contentType, signal));
      if (reference.contentHash.toLowerCase() !== expected) {
        throw new OriginSdkError({
          source: "apps", domain: "content", code: "content_commitment_mismatch",
          message: "Host preimage commitment does not match the canonical Commons content hash",
        });
      }
      return { commitment: expected, address };
    },
    async get(commitment, signal) {
      contentCommitment(commitment);
      const bytes = throwSdkResult(await host.getPreimage(commitment as `0x${string}`, signal));
      const actual = hashFromAddress(rawContentAddress(bytes, "blake2b-256"));
      if (actual !== commitment) {
        throw new OriginSdkError({
          source: "apps", domain: "content", code: "content_integrity",
          message: "Manifest bytes do not match the native Names content commitment",
        });
      }
      return bytes;
    },
  };
}

export function createOriginAppsClient(
  chain: CommonsChainClient,
  runtime: NamesRuntimeAdapter,
  content: OriginAppContentStore,
): OriginAppsClient {
  return {
    async storeManifest(manifest, signal) {
      try {
        const normalized = normalizeManifest(manifest);
        const bytes = encodeOriginAppManifest(normalized);
        const stored = await content.put(bytes, "application/vnd.cord.origin-app+json", signal);
        return ok({ manifest: normalized, bytes, ...stored });
      } catch (error) {
        return err(asSdkError(error, {
          source: "apps", domain: "manifest.store", code: "manifest_store_failed", retryable: true,
        }));
      }
    },
    async prepareManifestBinding(stored, signal) {
      try {
        const normalized = normalizeManifest(stored.manifest);
        const bytes = encodeOriginAppManifest(normalized);
        const expected = hashFromAddress(rawContentAddress(bytes, "blake2b-256"));
        if (expected !== stored.commitment) {
          return appError("manifest_commitment_mismatch", "Stored manifest commitment is not canonical");
        }
        const snapshot = await chain.finalizedSnapshot(signal);
        if (!snapshot.success) return snapshot;
        return ok(await runtime.setContent(
          snapshot.value.block.hash,
          normalized.nameId,
          stored.commitment,
          signal,
        ));
      } catch (error) {
        return err(asSdkError(error, {
          source: "apps", domain: "publication.bind", code: "binding_failed", retryable: false,
        }));
      }
    },
    async resolveApp(name, signal) {
      try { nameId(name); }
      catch (error) { return appError("invalid_name", error instanceof Error ? error.message : "Invalid name"); }
      const snapshot = await chain.finalizedSnapshot(signal);
      if (!snapshot.success) return snapshot;
      const at = snapshot.value.block.hash;
      const status = await snapshot.value.read((hash) => runtime.nameStatus(hash, name, signal), signal);
      if (!status.success) return status;
      if (!status.value.exists) return appError("name_not_found", "Application name does not exist");
      if (!status.value.active) return appError("name_inactive", "Application name is expired or inactive");
      const record = await snapshot.value.read((hash) => runtime.nameById(hash, name, signal), signal);
      if (!record.success) return record;
      if (record.value.value === null) return appError("name_not_found", "Application name record is missing");
      const linkedContent = await snapshot.value.read((hash) => runtime.resolveContent(hash, name, signal), signal);
      if (!linkedContent.success) return linkedContent;
      if (linkedContent.value.value === null) return appError("app_not_published", "Application name has no manifest commitment");
      const linkedAttestation = await snapshot.value.read(
        (hash) => runtime.resolveAttestation(hash, name, signal), signal,
      );
      if (!linkedAttestation.success) return linkedAttestation;
      try {
        const bytes = await content.get(linkedContent.value.value, signal);
        const manifest = decodeOriginAppManifest(bytes);
        if (manifest.nameId !== name) {
          return appError("manifest_name_mismatch", "Manifest does not belong to the resolved native name");
        }
        if (manifest.attestation !== undefined
          && linkedAttestation.value.value !== manifest.attestation) {
          return appError("attestation_not_live", "Manifest attestation is not live for the resolved name");
        }
        return ok({
          manifest,
          manifestCommitment: linkedContent.value.value,
          owner: record.value.value.owner,
          finalized: { hash: at, number: snapshot.value.block.number },
          launch: {
            isolation: "product-origin",
            productId: manifest.product.id,
            entrypoint: manifest.entrypoint,
            bundle: { ...manifest.bundle.address },
            requestedCapabilities: [...manifest.requestedCapabilities],
          },
        });
      } catch (error) {
        return err(asSdkError(error, {
          source: "apps", domain: "resolution.content", code: "manifest_resolution_failed", retryable: true,
        }));
      }
    },
  };
}

export const ORIGIN_APPS_CONTRACT = {
  schema: ORIGIN_APP_MANIFEST_SCHEMA,
  version: 1,
  ownershipAuthority: "Commons Names",
  contentAuthority: "Commons retained content commitment",
  discoveryAuthority: false,
  contractAddressAccepted: false,
  finalizedSnapshot: true,
} as const;

export * from "./deployment.ts";
export * from "./discovery.ts";
