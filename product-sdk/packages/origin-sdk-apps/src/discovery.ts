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


import { asSdkError, type SdkResult } from "@cord-network/origin-sdk-errors";
import { nameId, type ContentCommitment, type NameId } from "@cord-network/origin-sdk-names";
import { err, ok } from "@cord-network/origin-sdk-result";
import type { OriginAppsClient, ResolvedOriginApp } from "./index.ts";

export interface OriginAppIndexDocument {
  readonly schema: "cord.origin.app-index";
  readonly version: 1;
  readonly nameId: NameId;
  readonly manifestCommitment: ContentCommitment;
  readonly productId: string;
  readonly displayName: string;
  readonly channel: string;
  readonly categories: readonly string[];
  readonly visibility: "public" | "unlisted";
  readonly finalizedHash: `0x${string}`;
}

export interface OriginAppSearchRequest {
  readonly text?: string;
  readonly categories?: readonly string[];
  readonly limit?: number;
}

export interface OriginAppDiscoveryTransport {
  put(document: OriginAppIndexDocument, signal?: AbortSignal): Promise<void>;
  remove(name: NameId, signal?: AbortSignal): Promise<void>;
  search(request: Required<OriginAppSearchRequest>, signal?: AbortSignal): Promise<readonly OriginAppIndexDocument[]>;
}

export interface DiscoveredOriginApp {
  readonly indexed: OriginAppIndexDocument;
  readonly resolved: ResolvedOriginApp;
}

const category = (value: string): string => {
  if (!/^[a-z0-9][a-z0-9-]{0,31}$/.test(value)) throw new TypeError(`invalid app category: ${value}`);
  return value;
};

function searchRequest(input: OriginAppSearchRequest): Required<OriginAppSearchRequest> {
  const text = input.text?.trim().toLowerCase() ?? "";
  if (text.length > 128) throw new TypeError("app search text exceeds 128 characters");
  const categories = (input.categories ?? []).map(category);
  if (categories.length > 8 || new Set(categories).size !== categories.length) {
    throw new TypeError("app search categories must contain at most eight unique values");
  }
  const limit = input.limit ?? 20;
  if (!Number.isSafeInteger(limit) || limit < 1 || limit > 100) {
    throw new TypeError("app search limit must be 1-100");
  }
  return { text, categories, limit };
}

export function createOriginAppDiscoveryClient(
  apps: OriginAppsClient,
  transport: OriginAppDiscoveryTransport,
) {
  return {
    async publish(
      name: NameId,
      input: {
        readonly displayName: string;
        readonly categories?: readonly string[];
        readonly visibility?: "public" | "unlisted";
      },
      signal?: AbortSignal,
    ): Promise<SdkResult<OriginAppIndexDocument>> {
      try {
        nameId(name);
        if (!input.displayName.trim() || input.displayName.length > 128) {
          throw new TypeError("app display name must contain 1-128 characters");
        }
        const categories = (input.categories ?? []).map(category).sort();
        if (categories.length > 8 || new Set(categories).size !== categories.length) {
          throw new TypeError("app categories must contain at most eight unique values");
        }
        const resolved = await apps.resolveApp(name, signal);
        if (!resolved.success) return resolved;
        const document: OriginAppIndexDocument = {
          schema: "cord.origin.app-index",
          version: 1,
          nameId: name,
          manifestCommitment: resolved.value.manifestCommitment,
          productId: resolved.value.manifest.product.id,
          displayName: input.displayName.trim(),
          channel: resolved.value.manifest.channel,
          categories,
          visibility: input.visibility ?? "public",
          finalizedHash: resolved.value.finalized.hash,
        };
        await transport.put(document, signal);
        return ok(document);
      } catch (error) {
        return err(asSdkError(error, {
          source: "apps", domain: "discovery.publish", code: "index_publish_failed", retryable: true,
        }));
      }
    },
    async retract(name: NameId, signal?: AbortSignal): Promise<SdkResult<void>> {
      try { nameId(name); await transport.remove(name, signal); return ok(undefined); }
      catch (error) {
        return err(asSdkError(error, {
          source: "apps", domain: "discovery.retract", code: "index_retract_failed", retryable: true,
        }));
      }
    },
    async search(
      input: OriginAppSearchRequest = {},
      signal?: AbortSignal,
    ): Promise<SdkResult<readonly DiscoveredOriginApp[]>> {
      try {
        const request = searchRequest(input);
        const indexed = await transport.search(request, signal);
        const current: DiscoveredOriginApp[] = [];
        for (const document of indexed.slice(0, request.limit)) {
          if (document.schema !== "cord.origin.app-index" || document.version !== 1
            || document.visibility !== "public") continue;
          const resolved = await apps.resolveApp(document.nameId, signal);
          if (!resolved.success || resolved.value.manifestCommitment !== document.manifestCommitment
            || resolved.value.manifest.product.id !== document.productId) continue;
          current.push({ indexed: { ...document, categories: [...document.categories] }, resolved: resolved.value });
        }
        return ok(current);
      } catch (error) {
        return err(asSdkError(error, {
          source: "apps", domain: "discovery.search", code: "index_search_failed", retryable: true,
        }));
      }
    },
  };
}

export function createMemoryOriginAppDiscoveryTransport(): OriginAppDiscoveryTransport {
  const documents = new Map<string, OriginAppIndexDocument>();
  return {
    async put(document) { documents.set(document.nameId, { ...document, categories: [...document.categories] }); },
    async remove(name) { documents.delete(name); },
    async search(request) {
      return [...documents.values()].filter((document) => {
        const haystack = `${document.displayName} ${document.productId}`.toLowerCase();
        return document.visibility === "public"
          && (!request.text || haystack.includes(request.text))
          && request.categories.every((item) => document.categories.includes(item));
      }).sort((left, right) => left.nameId < right.nameId ? -1 : left.nameId > right.nameId ? 1 : 0)
        .slice(0, request.limit)
        .map((document) => ({ ...document, categories: [...document.categories] }));
    },
  };
}
