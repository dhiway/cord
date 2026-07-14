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
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import ts from "typescript";

const root = resolve(import.meta.dirname, "../..");
const input = resolve(root, "product-sdk/packages/descriptors/chains/commons/generated/dist/commons.d.ts");
const output = resolve(root, "docs/sdk/commons-papi-app-surface.json");
const metadataIdentity = JSON.parse(
  readFileSync(resolve(root, "docs/sdk/metadata/commons-v29.json"), "utf8"),
);
const source = readFileSync(input, "utf8");
const file = ts.createSourceFile(input, source, ts.ScriptTarget.Latest, true, ts.ScriptKind.TS);
const typeNames = { storage: "IStorage", calls: "ICalls", events: "IEvent", errors: "IError", constants: "IConstants" } as const;
const pallets = ["People", "PeopleLite", "Personhood", "Resources", "Attestation", "Names", "TransactionStorage", "StorageProvider", "Drive", "S3", "Assets", "Uniques", "Nfts", "MetaTx", "Revive"] as const;
const propertyName = (member: ts.TypeElement): string | undefined => member.name && ts.isIdentifier(member.name)
  ? member.name.text
  : member.name && ts.isStringLiteral(member.name) ? member.name.text : undefined;
const types = new Map<string, ts.TypeLiteralNode>();
for (const statement of file.statements) {
  if (ts.isTypeAliasDeclaration(statement) && ts.isTypeLiteralNode(statement.type))
    types.set(statement.name.text, statement.type);
}
const surface: Record<string, Record<string, string[]>> = {};
for (const pallet of pallets) {
  surface[pallet] = {};
  for (const [kind, typeName] of Object.entries(typeNames)) {
    const container = types.get(typeName);
    const member = container?.members.find((candidate) => propertyName(candidate) === pallet);
    const nested = member && ts.isPropertySignature(member) && member.type && ts.isTypeLiteralNode(member.type)
      ? member.type : undefined;
    surface[pallet][kind] = nested?.members.map(propertyName).filter((name): name is string => name !== undefined) ?? [];
  }
}
const routes = JSON.parse(readFileSync(resolve(root, "docs/sdk/native-route-contract.json"), "utf8"));
const manifest = {
  schema: "cord.commons-papi-app-surface.v1",
  metadata_scale_sha256: metadataIdentity.scale_sha256,
  generated_declaration_sha256: createHash("sha256").update(source).digest("hex"),
  policy_route_count: routes.route_count,
  policy_routes_by_capability: Object.fromEntries([...new Set(routes.routes.map((route: any) => route.capability))]
    .sort().map((capability) => [capability, routes.routes.filter((route: any) => route.capability === capability).map((route: any) => route.id)])),
  app_pallets: surface,
  boundary: {
    native_pallets_authoritative: true,
    metadata_presence_is_not_sdk_admission: true,
    administrative_calls_require_explicit_exclusion: true,
    revive_is_optional_app_logic_only: true,
  },
};
const serialized = `${JSON.stringify(manifest, null, 2)}\n`;
if (process.argv.includes("--check")) {
  if (!existsSync(output) || readFileSync(output, "utf8") !== serialized)
    throw new Error("Commons PAPI app surface drift; run npm run generate:inventory");
  process.stdout.write(`PASS Commons PAPI app surface: pallets=${pallets.length} policy_routes=${routes.route_count}\n`);
} else {
  writeFileSync(output, serialized);
  process.stdout.write(`${output}\n`);
}
