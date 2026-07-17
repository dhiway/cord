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
  base64Content,
  decimalU64,
  parseContentCid,
  rawContentAddress,
  storageRequests,
  type CloudStorageClient,
  type ContentAddress,
} from "@cord-network/origin-sdk-cloud-storage";
import { OriginSdkError, asSdkError, type SdkResult } from "@cord-network/origin-sdk-errors";
import type { OriginHostClient, ProductIdentity } from "@cord-network/origin-sdk-host";
import { contentCommitment, type AttestationId, type BlockNumber, type ContentCommitment, type NameId, type OperationId } from "@cord-network/origin-sdk-names";
import { err, ok } from "@cord-network/origin-sdk-result";
import type { PreparedTransaction } from "@cord-network/origin-sdk-tx";
import {
  ORIGIN_APP_MANIFEST_SCHEMA,
  type OriginAppManifestV1,
  type OriginAppsClient,
  type OriginRequestedCapability,
  type StoredOriginAppManifest,
} from "./index.ts";

export interface OriginStaticFile {
  readonly path: string;
  readonly bytes: Uint8Array;
  readonly mediaType?: string;
}

export interface OriginBundleBlock {
  readonly address: ContentAddress;
  readonly commitment: ContentCommitment;
  readonly bytes: Uint8Array;
  readonly mediaType: string;
}

export interface OriginPackagedBundle {
  readonly format: "origin-static-v1" | "car-unixfs";
  readonly root: ContentAddress;
  readonly size: number;
  readonly blocks: readonly OriginBundleBlock[];
}

export interface OriginBundlePackager {
  readonly format: OriginPackagedBundle["format"];
  package(files: readonly OriginStaticFile[]): Promise<OriginPackagedBundle>;
}

export interface OriginAppBlockStore {
  has(commitment: ContentCommitment, signal?: AbortSignal): Promise<boolean>;
  put(block: OriginBundleBlock, signal?: AbortSignal): Promise<void>;
}

export interface PrepareOriginAppDeployment {
  readonly product: ProductIdentity;
  readonly nameId: NameId;
  readonly version: string;
  readonly channel: string;
  readonly entrypoint: string;
  readonly contentFormat: "static" | "pwa";
  readonly requestedCapabilities: readonly OriginRequestedCapability[];
  readonly attestation?: AttestationId;
  readonly files: readonly OriginStaticFile[];
  readonly publication: { readonly expectedRevision: string; readonly operationDeadline: BlockNumber; readonly operationId: OperationId };
}

export interface PreparedOriginAppDeployment {
  readonly bundle: OriginPackagedBundle;
  readonly manifest: OriginAppManifestV1;
  readonly storedManifest: StoredOriginAppManifest;
  readonly retentionTransactions: readonly PreparedTransaction[];
  readonly uploadedCommitments: readonly ContentCommitment[];
  readonly reusedCommitments: readonly ContentCommitment[];
  /** Call only after every retention transaction has finalized. */
  prepareBinding(signal?: AbortSignal): Promise<SdkResult<PreparedTransaction>>;
}

const utf8 = new TextEncoder();
const text = new TextDecoder("utf-8", { fatal: true });

function commitmentOf(address: ContentAddress): ContentCommitment {
  return contentCommitment(`0x${Array.from(
    parseContentCid(address.cid).digest,
    (byte) => byte.toString(16).padStart(2, "0"),
  ).join("")}`);
}

function safePath(path: string): string {
  if (!path || utf8.encode(path).length > 256 || path.startsWith("/") || path.includes("\\")
    || path.split("/").some((part) => !part || part === "." || part === "..")
    || /[?#\0]/.test(path)) throw new TypeError(`unsafe application file path: ${path}`);
  return path;
}

function mediaType(value: string | undefined): string {
  const normalized = value ?? "application/octet-stream";
  if (!/^[a-z0-9!#$&^_.+-]+\/[a-z0-9!#$&^_.+-]+$/i.test(normalized) || normalized.length > 128) {
    throw new TypeError("application file media type is invalid");
  }
  return normalized.toLowerCase();
}

function base64(bytes: Uint8Array): string {
  const alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
  let output = "";
  for (let index = 0; index < bytes.length; index += 3) {
    const first = bytes[index] ?? 0;
    const second = bytes[index + 1] ?? 0;
    const third = bytes[index + 2] ?? 0;
    const value = (first << 16) | (second << 8) | third;
    output += alphabet[(value >>> 18) & 63];
    output += alphabet[(value >>> 12) & 63];
    output += index + 1 < bytes.length ? alphabet[(value >>> 6) & 63] : "=";
    output += index + 2 < bytes.length ? alphabet[value & 63] : "=";
  }
  return output;
}

/** Deterministic root index plus one raw CID block per application file. */
export function createOriginStaticPackager(): OriginBundlePackager {
  return {
    format: "origin-static-v1",
    async package(files) {
      if (files.length < 1 || files.length > 4_096) {
        throw new TypeError("application bundle must contain 1-4096 files");
      }
      const normalized = files.map((file) => ({
        path: safePath(file.path),
        bytes: file.bytes.slice(),
        mediaType: mediaType(file.mediaType),
      })).sort((left, right) => left.path < right.path ? -1 : left.path > right.path ? 1 : 0);
      if (new Set(normalized.map(({ path }) => path)).size !== normalized.length) {
        throw new TypeError("application bundle paths must be unique");
      }
      const total = normalized.reduce((size, file) => size + file.bytes.length, 0);
      if (total > 64 * 1024 * 1024) throw new TypeError("application bundle exceeds 67108864 bytes");
      const fileBlocks = normalized.map((file): OriginBundleBlock => {
        if (file.bytes.length < 1) throw new TypeError(`application file is empty: ${file.path}`);
        const address = rawContentAddress(file.bytes);
        return { address, commitment: commitmentOf(address), bytes: file.bytes, mediaType: file.mediaType };
      });
      const indexBytes = utf8.encode(JSON.stringify({
        schema: "cord.origin.static-bundle",
        version: 1,
        files: normalized.map((file, index) => ({
          path: file.path,
          mediaType: file.mediaType,
          size: file.bytes.length,
          address: fileBlocks[index]!.address,
        })),
      }));
      const root = rawContentAddress(indexBytes);
      const rootBlock: OriginBundleBlock = {
        address: root,
        commitment: commitmentOf(root),
        bytes: indexBytes,
        mediaType: "application/vnd.cord.origin-static+json",
      };
      return { format: "origin-static-v1", root, size: total, blocks: [...fileBlocks, rootBlock] };
    },
  };
}

/** Host transport adapter used only for content availability; runtime retention remains separate. */
export function createHostOriginAppBlockStore(host: OriginHostClient): OriginAppBlockStore {
  return {
    async has(commitment, signal) {
      const result = await host.getPreimage(commitment as `0x${string}`, signal);
      if (result.success) return true;
      if (result.error.code === "not_found") return false;
      throw result.error;
    },
    async put(block, signal) {
      const stored = await host.putPreimage(block.bytes, block.mediaType, signal);
      if (!stored.success) throw stored.error;
      if (stored.value.contentHash.toLowerCase() !== block.commitment) {
        throw new OriginSdkError({
          source: "apps", domain: "deployment.upload", code: "content_commitment_mismatch",
          message: "Uploaded block commitment does not match its canonical CID",
        });
      }
    },
  };
}

export function createOriginAppDeployer(
  apps: OriginAppsClient,
  storage: CloudStorageClient,
  blocks: OriginAppBlockStore,
  packager: OriginBundlePackager = createOriginStaticPackager(),
) {
  return {
    async prepare(
      input: PrepareOriginAppDeployment,
      signal?: AbortSignal,
    ): Promise<SdkResult<PreparedOriginAppDeployment>> {
      try {
        const bundle = await packager.package(input.files);
        if (!input.files.some(({ path }) => path === input.entrypoint)) {
          throw new TypeError("application entrypoint is not present in the bundle");
        }
        const retentionTransactions: PreparedTransaction[] = [];
        const uploadedCommitments: ContentCommitment[] = [];
        const reusedCommitments: ContentCommitment[] = [];
        for (const block of bundle.blocks) {
          if (await blocks.has(block.commitment, signal)) {
            reusedCommitments.push(block.commitment);
            continue;
          }
          await blocks.put(block, signal);
          const prepared = await storage.prepare(storageRequests.storeWithCidConfig(
            { codec: decimalU64(85), hashing: "blake2b256" },
            base64Content(base64(block.bytes)),
          ), signal);
          if (!prepared.success) return prepared;
          retentionTransactions.push(prepared.value);
          uploadedCommitments.push(block.commitment);
        }
        const manifest: OriginAppManifestV1 = {
          schema: ORIGIN_APP_MANIFEST_SCHEMA,
          schemaVersion: 1,
          product: { ...input.product },
          nameId: input.nameId,
          version: input.version,
          channel: input.channel,
          entrypoint: input.entrypoint,
          contentFormat: input.contentFormat,
          bundle: { address: bundle.root, size: bundle.size },
          requestedCapabilities: [...input.requestedCapabilities],
          ...(input.attestation === undefined ? {} : { attestation: input.attestation }),
        };
        const stored = await apps.storeManifest(manifest, signal);
        if (!stored.success) return stored;
        const manifestRetention = await storage.prepare(storageRequests.storeWithCidConfig(
          { codec: decimalU64(85), hashing: "blake2b256" },
          base64Content(base64(stored.value.bytes)),
        ), signal);
        if (!manifestRetention.success) return manifestRetention;
        retentionTransactions.push(manifestRetention.value);
        uploadedCommitments.push(stored.value.commitment);
        return ok({
          bundle,
          manifest: stored.value.manifest,
          storedManifest: stored.value,
          retentionTransactions,
          uploadedCommitments,
          reusedCommitments,
          prepareBinding: (bindingSignal) => apps.prepareManifestBinding(stored.value, input.publication, bindingSignal),
        });
      } catch (error) {
        return err(asSdkError(error, {
          source: "apps", domain: "deployment.prepare", code: "deployment_failed", retryable: true,
        }));
      }
    },
  };
}

export function decodeOriginStaticBundleIndex(bytes: Uint8Array): unknown {
  return JSON.parse(text.decode(bytes));
}
