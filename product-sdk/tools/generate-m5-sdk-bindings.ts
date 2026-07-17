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

import { createHash } from "node:crypto";
import { readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";

const root = resolve(import.meta.dirname, "../..");
const read = (path: string) => readFileSync(resolve(root, path), "utf8");
const json = (path: string) => JSON.parse(read(path));
const sha256 = (path: string) => createHash("sha256").update(readFileSync(resolve(root, path))).digest("hex");
const fail = (message: string): never => { throw new Error(message); };

const mapPath = "docs/sdk/contract-to-native-map.json";
const routePath = "docs/sdk/native-route-contract.json";
const vectorPath = "docs/sdk/vectors/names-v1.json";
const outputPath = "docs/sdk/m5-sdk-bindings.json";
const coverage = json(mapPath);
const routeContract = json(routePath);
const routeIds = new Set(routeContract.routes.map((route: any) => route.id));

const exactRoutes: Record<string, readonly string[]> = {
  "function:owner": ["names:name_by_id"], "function:recordExists": ["names:name_status"],
  "function:contenthash": ["names:resolve_content_publication"], "function:text": ["names:resolve_text"],
  "function:addressOf": ["names:resolve_address"], "function:nameOf": ["names:primary_name"],
  "function:isSingleLabel": ["names:register", "names:reserve_name", "names:set_label_protection"],
  "function:isSingleLabelMemory": ["names:register", "names:reserve_name", "names:set_label_protection"],
};

const adopted = coverage.entries.filter((entry: any) => entry.disposition === "adopt-semantic");
const keyOf = (entry: any) => `${entry.source_symbol_kind}:${entry.source_symbol}`;
const bindings = adopted.map((entry: any) => {
  if (entry.source_symbol_kind !== "function") fail(`adopted non-function has no M5 binding: ${entry.source_id}#${keyOf(entry)}`);
  // Source identifiers remain immutable provenance; only the native target surface is branded.
  const validationSurface = entry.source_id === "dotns:contracts/utils/StringUtils.sol"
    && ["isSingleLabel", "isSingleLabelMemory"].includes(entry.source_symbol);
  const bindingKind = validationSurface ? "label-validation-surface" : "native-route-set";
  const routes = exactRoutes[keyOf(entry)] ?? fail(`missing explicit route mapping for ${entry.source_id}#${keyOf(entry)}`);
  for (const route of routes) if (!routeIds.has(route)) fail(`unknown route ${route}`);
  const binding: any = {
    key: {
      source_id: entry.source_id,
      source_symbol_kind: entry.source_symbol_kind,
      source_symbol: entry.source_symbol,
    },
    binding_kind: bindingKind,
    behavior_concept: validationSurface ? "single-label-validation" : entry.source_symbol,
    navigation_hint: {
      rust: entry.rust_sdk.replaceAll("DotnsQuery", "NamesQuery"),
      typescript: entry.typescript_sdk.replace(/^dotns\b/, "names"),
    },
    routes,
    exercised_by: {
      rust: {
        path: "origin-rs/tests/product_sdk_native.rs",
        case: "every_authoritative_native_route_constructs_validates_and_dispatches_in_rust",
      },
      typescript: {
        path: "product-sdk/tests/conformance/native-route-harness.test.ts",
        case: "every authoritative native route constructs, validates and selects its exact TS dispatch",
      },
    },
  };
  if (bindingKind === "label-validation-surface") {
    binding.validation_surfaces = {
      rust: { path: "origin-rs/src/product_sdk/domains/names.rs", declaration: "Label", constructor: "new" },
      typescript: { path: "product-sdk/packages/origin-sdk-names/src/index.ts", function: "normalizedLabel" },
      vectors: { path: vectorPath, sha256: sha256(vectorPath) },
      cases: {
        rust: { path: "origin-rs/src/product_sdk/domains/names.rs", case: "shared_vectors_match_label_name_commitment_and_event_contracts" },
        typescript: { path: "product-sdk/tests/conformance/names-vectors.test.ts", case: "shared Orbis Names canonical vectors match TypeScript SDK" },
      },
    };
  }
  return binding;
});

bindings.sort((left: any, right: any) => JSON.stringify(left.key).localeCompare(JSON.stringify(right.key)));
const distinctBehaviorConcepts = new Set(bindings.map((binding: any) => binding.behavior_concept)).size;
const artifact = {
  schema: "cord.m5-sdk-bindings.v1",
  status: coverage.approval_manifest.verdict === "APPROVED"
    ? "authoritative" : "structural-current-approval-pending",
  purpose: "Exact M5 bindings for architect-approved adopt-semantic source symbols; navigation hints in the design map are non-normative and unmappable adopted rows fail closed.",
  inputs: {
    design_map: { path: mapPath, sha256: sha256(mapPath) },
    route_contract: { path: routePath, sha256: sha256(routePath) },
    approval_manifest: coverage.approval_manifest,
  },
  counts: {
    design_entries: coverage.entries.length,
    adopted_semantics: adopted.length,
    exact_bindings: bindings.length,
    distinct_behavior_concepts: distinctBehaviorConcepts,
    blockers: 0,
    blocker_categories: {},
  },
  bindings,
  blockers: [],
};
const serialized = `${JSON.stringify(artifact, null, 2)}\n`;
if (process.argv.includes("--check")) {
  if (read(outputPath) !== serialized) fail("M5 SDK binding artifact is stale");
  process.stdout.write(`${coverage.approval_manifest.verdict === "APPROVED" ? "PASS" : "PENDING_APPROVAL"} M5 SDK bindings current: ${bindings.length} exact bindings, 0 blockers\n`);
} else {
  writeFileSync(resolve(root, outputPath), serialized);
  process.stdout.write(`${outputPath}\n`);
}
