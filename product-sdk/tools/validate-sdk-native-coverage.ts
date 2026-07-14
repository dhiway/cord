import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { NATIVE_HOST_METHODS } from "../packages/descriptors/src/native-methods.ts";
import { NATIVE_RUNTIME_ROUTE_REGISTRY } from "../packages/descriptors/src/runtime-route-registry.ts";
import { ORBIS_CANDIDATE_NETWORK_BINDING, ORBIS_NETWORK_BINDING } from "../packages/descriptors/generated/orbis-network-binding.ts";
import { canonicalMethodPayload, methodPayloadSchema } from "../packages/core/src/contract.ts";
import { NATIVE_SDK_VERSION } from "../src/version.ts";

const root = resolve(import.meta.dirname, "../..");
const read = (path: string) => readFileSync(resolve(root, path), "utf8");
const json = (path: string) => JSON.parse(read(path));
const sha256 = (path: string) => createHash("sha256").update(readFileSync(resolve(root, path))).digest("hex");
const fail = (message: string): never => { throw new Error(message); };
const equal = (actual: unknown, expected: unknown, label: string) => {
  if (actual !== expected) fail(`${label}: expected ${String(expected)}, found ${String(actual)}`);
};
const firstVersion = (source: string, field: "spec_version" | "transaction_version") => {
  const match = source.match(new RegExp(`${field}:\\s*(\\d+)`));
  return match ? Number(match[1]) : fail(`missing ${field}`);
};
const storageVersion = (path: string) => {
  const match = read(path).match(/StorageVersion::new\((\d+)\)/);
  return match ? Number(match[1]) : fail(`missing storage version in ${path}`);
};
const rustConstant = (source: string, name: string): string => {
  const match = source.match(new RegExp(`pub const ${name}:[^=]+=[\\s\\n]*(?:\\n?\\s*)?[\"']?([^;\"']+)[\"']?;`));
  if (!match) fail(`missing Rust version constant ${name}`);
  return match[1].trim().replace(/_/g, "");
};
const skipRustTrivia = (source: string, start: number): number => {
  let cursor = start;
  while (cursor < source.length) {
    if (/\s/.test(source[cursor])) { cursor++; continue; }
    if (source.startsWith("//", cursor)) {
      const end = source.indexOf("\n", cursor + 2);
      cursor = end < 0 ? source.length : end + 1;
      continue;
    }
    if (source.startsWith("/*", cursor)) {
      const end = source.indexOf("*/", cursor + 2);
      if (end < 0) fail("unterminated Rust block comment while parsing pallet calls");
      cursor = end + 2;
      continue;
    }
    break;
  }
  return cursor;
};
const skipRustAttribute = (source: string, start: number): number => {
  if (!source.startsWith("#[", start)) fail("expected Rust attribute");
  let depth = 0;
  let quote: string | undefined;
  for (let cursor = start + 1; cursor < source.length; cursor++) {
    const character = source[cursor];
    if (quote) {
      if (character === "\\") cursor++;
      else if (character === quote) quote = undefined;
      continue;
    }
    if (character === '"' || character === "'") { quote = character; continue; }
    if (character === "[") depth++;
    else if (character === "]" && --depth === 0) return cursor + 1;
  }
  return fail("unterminated Rust attribute while parsing pallet calls");
};
const parsePalletCallItems = (source: string, path: string): Map<string, number> => {
  const calls = new Map<string, number>();
  const marker = "#[pallet::call_index(";
  let searchFrom = 0;
  while (true) {
    const start = source.indexOf(marker, searchFrom);
    if (start < 0) break;
    const indexMatch = source.slice(start).match(/^#\[pallet::call_index\((\d+)\)\]/);
    if (!indexMatch) fail(`malformed pallet call index attribute in ${path}`);
    let cursor = skipRustTrivia(source, start + indexMatch[0].length);
    while (source.startsWith("#[", cursor)) {
      cursor = skipRustTrivia(source, skipRustAttribute(source, cursor));
    }
    const functionMatch = source.slice(cursor).match(/^pub\s+fn\s+([a-z][a-z0-9_]*)\s*\(/);
    if (!functionMatch) fail(`call index ${indexMatch[1]} in ${path} is not attached to the immediately following pub fn item`);
    const name = functionMatch[1];
    if (calls.has(name)) fail(`duplicate pallet call item ${path}::${name}`);
    calls.set(name, Number(indexMatch[1]));
    searchFrom = cursor + functionMatch[0].length;
  }
  return calls;
};

const matrix = json("docs/sdk/native-version-matrix.json");
const metadata = json(matrix.networks.orbis.metadata_source);
const candidateGenesis = json(matrix.networks.orbis.candidate_genesis_identity_source);
const vectorBaselineManifest = json("origin/orbis/runtime/vectors/transaction-policy-v8/manifest.json");
const vectorBaselineMetadata = json("origin/orbis/runtime/vectors/transaction-policy-v8/metadata-hash-vector-baseline.json");
const vectors = json(matrix.semantic_vectors);
const coverage = json(matrix.coverage_map);
const m5Bindings = json("docs/sdk/m5-sdk-bindings.json");
const routeContract = json("docs/sdk/native-route-contract.json");
const descriptor = json("product-sdk/packages/descriptors/generated/orbis-descriptor.json");
const ratification = json(matrix.activation_source);
const rust = read("origin-rs/src/product_sdk/version.rs");
const rustAttestation = read("origin-rs/src/product_sdk/domains/attestation.rs");
const rustNames = read("origin-rs/src/product_sdk/domains/names.rs");
const rustStorage = read("origin-rs/src/product_sdk/domains/storage.rs");
const rustStorageProvider = read("origin-rs/src/product_sdk/domains/storage_provider.rs");
const rustDrive = read("origin-rs/src/product_sdk/domains/drive.rs");
const rustS3 = read("origin-rs/src/product_sdk/domains/s3.rs");
const rustIdentityPersonhood = read("origin-rs/src/product_sdk/domains/identity_personhood.rs");
const rustSponsoredIntent = read("origin-rs/src/product_sdk/sponsored_intent.rs");
const rustContract = read("origin-rs/src/product_sdk/contract.rs");
const rustCommon = read("origin-rs/src/product_sdk/domains/common.rs");
const rustSurface = `${rust}\n${rustAttestation}\n${rustNames}\n${rustStorage}\n${rustStorageProvider}\n${rustDrive}\n${rustS3}\n${rustIdentityPersonhood}\n${rustSponsoredIntent}\n${rustCommon}\n${rustContract}`;
const originRuntime = read(matrix.networks.origin.runtime_source);
const orbisRuntime = read(matrix.networks.orbis.runtime_source);
const workspaceVersion = read("Cargo.toml").match(/^version\s*=\s*"([^"]+)"/m)?.[1];

equal(matrix.schema, "cord.native-sdk-version-matrix.v1", "matrix schema");
equal(matrix.branch, "sm-update-sub-0x63", "matrix branch");
equal(matrix.clean_break.new_network, true, "clean-break new network");
for (const field of ["backward_compatibility", "data_migration", "legacy_client", "contract_compatibility_facade"])
  equal(matrix.clean_break[field], false, `clean-break ${field}`);
equal(workspaceVersion, matrix.packages.rust.version, "Rust package version");
equal(json("product-sdk/package.json").version, matrix.packages.typescript.version, "TypeScript package version");

equal(firstVersion(originRuntime, "spec_version"), matrix.networks.origin.spec_version, "Origin spec version");
equal(firstVersion(originRuntime, "transaction_version"), matrix.networks.origin.transaction_version, "Origin transaction version");
equal(firstVersion(orbisRuntime, "spec_version"), matrix.networks.orbis.spec_version, "Orbis spec version");
equal(firstVersion(orbisRuntime, "transaction_version"), matrix.networks.orbis.transaction_version, "Orbis transaction version");
equal(matrix.networks.origin.activation_state, "candidate-pending", "Origin candidate activation state");
equal(matrix.networks.origin.production_activation_ready, false, "Origin candidate production gate");
equal(metadata.metadata_hash, matrix.networks.orbis.metadata_hash, "Orbis metadata hash");
equal(metadata.compact_wasm_sha256, "558727456824d1f06928148ffead8cee5d58a63bf9109d0334a061f30c35db0f", "Orbis compact Wasm");
equal(sha256(matrix.networks.orbis.candidate_genesis_identity_source), matrix.networks.orbis.candidate_genesis_identity_sha256, "candidate genesis artifact");
equal(candidateGenesis.genesis.header_hash, matrix.networks.orbis.candidate_genesis_header_hash, "candidate genesis header");
equal(candidateGenesis.genesis.state_root, matrix.networks.orbis.candidate_genesis_state_root, "candidate genesis state root");
equal(candidateGenesis.runtime.metadata_hash, metadata.metadata_hash, "candidate genesis metadata");
equal(candidateGenesis.runtime.metadata_hash_manifest_sha256, sha256(matrix.networks.orbis.metadata_source), "candidate metadata manifest binding");
equal(candidateGenesis.runtime.compact_wasm_sha256, metadata.compact_wasm_sha256, "candidate compact Wasm binding");
equal(candidateGenesis.production_activation, false, "candidate production activation");
equal(matrix.networks.orbis.activation_state, "candidate-pending", "Orbis candidate activation state");
equal(matrix.networks.orbis.production_activation_ready, false, "Orbis candidate production gate");
equal(ratification.payload.production_activation.final_genesis_status, "PENDING", "P5 final genesis status");
equal(ratification.derived_status.production_activation_ready, false, "P5 production activation gate");
equal(vectorBaselineManifest.fixture_status, "historical-signed-payload-vector-baseline", "signed-payload vector status");
equal(vectorBaselineManifest.current_runtime_vectors_regenerated, false, "signed-payload current-runtime claim");
equal(vectorBaselineManifest.metadata_record, "metadata-hash-vector-baseline.json", "signed-payload metadata record");
equal(vectorBaselineManifest.current_runtime_metadata_record, "metadata-hash.json", "current metadata record");
equal(vectorBaselineManifest.metadata_implicit, vectorBaselineMetadata.metadata_hash, "signed-payload baseline metadata");
const baselineConstant = read("origin/orbis/runtime/src/transaction_policy_vectors.rs").match(/const VECTOR_BASELINE_METADATA_IMPLICIT:[^=]+=\s*\[([\s\S]*?)\];/)?.[1];
if (!baselineConstant) fail("missing historical vector metadata constant");
const baselineConstantHex = `0x${[...baselineConstant.matchAll(/0x([0-9a-f]{2})/g)].map((match) => match[1]).join("")}`;
equal(baselineConstantHex, vectorBaselineManifest.metadata_implicit, "runtime vector baseline constant");
if (vectorBaselineManifest.metadata_implicit === metadata.metadata_hash) fail("historical vector baseline must not be represented as current metadata");

equal(NATIVE_SDK_VERSION.contractVersion, matrix.contract_version, "TypeScript contract version");
equal(NATIVE_SDK_VERSION.release, matrix.release, "TypeScript release");
equal(NATIVE_SDK_VERSION.sdkRelease, matrix.sdk_release, "TypeScript SDK release");
equal(NATIVE_SDK_VERSION.origin.specVersion, matrix.networks.origin.spec_version, "TypeScript Origin spec");
equal(NATIVE_SDK_VERSION.origin.transactionVersion, matrix.networks.origin.transaction_version, "TypeScript Origin transaction");
equal(NATIVE_SDK_VERSION.origin.activationState, matrix.networks.origin.activation_state, "TypeScript Origin activation state");
equal(NATIVE_SDK_VERSION.origin.productionActivationReady, false, "TypeScript Origin production gate");
equal(NATIVE_SDK_VERSION.orbis.paraId, matrix.networks.orbis.para_id, "TypeScript Orbis para ID");
equal(NATIVE_SDK_VERSION.orbis.specVersion, matrix.networks.orbis.spec_version, "TypeScript Orbis spec");
equal(NATIVE_SDK_VERSION.orbis.transactionVersion, matrix.networks.orbis.transaction_version, "TypeScript Orbis transaction");
equal(NATIVE_SDK_VERSION.orbis.metadataHash, matrix.networks.orbis.metadata_hash, "TypeScript Orbis metadata");
equal(NATIVE_SDK_VERSION.orbis.candidateGenesisHeaderHash, matrix.networks.orbis.candidate_genesis_header_hash, "TypeScript Orbis candidate genesis");
equal(NATIVE_SDK_VERSION.orbis.candidateGenesisStateRoot, matrix.networks.orbis.candidate_genesis_state_root, "TypeScript Orbis candidate state root");
equal(NATIVE_SDK_VERSION.orbis.candidateGenesisIdentitySha256, matrix.networks.orbis.candidate_genesis_identity_sha256, "TypeScript candidate artifact");
equal(NATIVE_SDK_VERSION.orbis.activationState, matrix.networks.orbis.activation_state, "TypeScript Orbis activation state");
equal(NATIVE_SDK_VERSION.orbis.productionActivationReady, matrix.networks.orbis.production_activation_ready, "TypeScript Orbis production gate");
equal(ORBIS_NETWORK_BINDING.activation_state, matrix.networks.orbis.activation_state, "generated binding activation state");
equal(ORBIS_NETWORK_BINDING.production_activation_ready, false, "generated binding production gate");
equal(ORBIS_CANDIDATE_NETWORK_BINDING.access_mode, "candidate", "explicit candidate binding opt-in");

for (const [name, value] of [
  ["NATIVE_SDK_CONTRACT_VERSION", matrix.contract_version],
  ["ORIGIN_SPEC_VERSION", matrix.networks.origin.spec_version],
  ["ORIGIN_TRANSACTION_VERSION", matrix.networks.origin.transaction_version],
  ["ORBIS_PARA_ID", matrix.networks.orbis.para_id],
  ["ORBIS_SPEC_VERSION", matrix.networks.orbis.spec_version],
  ["ORBIS_TRANSACTION_VERSION", matrix.networks.orbis.transaction_version],
] as const) equal(Number(rustConstant(rust, name)), value, `Rust ${name}`);
equal(rustConstant(rust, "NATIVE_SDK_RELEASE"), matrix.release, "Rust native release");
equal(rustConstant(rust, "NATIVE_SDK_PACKAGE_VERSION"), matrix.sdk_release, "Rust package release");
equal(rustConstant(rust, "ORBIS_METADATA_HASH"), matrix.networks.orbis.metadata_hash, "Rust Orbis metadata");
equal(rustConstant(rust, "ORIGIN_ACTIVATION_STATE"), matrix.networks.origin.activation_state, "Rust Origin activation state");
equal(rustConstant(rust, "ORIGIN_PRODUCTION_ACTIVATION_READY"), "false", "Rust Origin production gate");
equal(rustConstant(rust, "ORBIS_ACTIVATION_STATE"), matrix.networks.orbis.activation_state, "Rust Orbis activation state");
equal(rustConstant(rust, "ORBIS_PRODUCTION_ACTIVATION_READY"), "false", "Rust Orbis production gate");
equal(rustConstant(rust, "ORBIS_CANDIDATE_GENESIS_HEADER_HASH"), matrix.networks.orbis.candidate_genesis_header_hash, "Rust Orbis candidate genesis");
equal(rustConstant(rust, "ORBIS_CANDIDATE_GENESIS_STATE_ROOT"), matrix.networks.orbis.candidate_genesis_state_root, "Rust Orbis candidate state root");
equal(rustConstant(rust, "ORBIS_CANDIDATE_GENESIS_IDENTITY_SHA256"), matrix.networks.orbis.candidate_genesis_identity_sha256, "Rust candidate artifact");

const apiVersions = [...read("origin/orbis/runtime-api/storage/src/lib.rs").matchAll(/#\[api_version\((\d+)\)\]/g)].map((match) => Number(match[1]));
const observedApis = {
  identity_personhood: Number(read(matrix.native_runtime_apis.identity_personhood.source).match(/#\[api_version\((\d+)\)\]/)?.[1]),
  attestation: Number(read(matrix.native_runtime_apis.attestation.source).match(/#\[api_version\((\d+)\)\]/)?.[1]),
  names: Number(read(matrix.native_runtime_apis.names.source).match(/#\[api_version\((\d+)\)\]/)?.[1]),
  storage_provider: apiVersions[0],
  drive: apiVersions[1],
  s3: apiVersions[2],
};
const apiBindings = {
  identity_personhood: ["identityPersonhood", "IDENTITY_PERSONHOOD_RUNTIME_API_VERSION"],
  attestation: ["attestation", "ATTESTATION_RUNTIME_API_VERSION"],
  names: ["names", "NAMES_RUNTIME_API_VERSION"],
  storage_provider: ["storageProvider", "STORAGE_PROVIDER_RUNTIME_API_VERSION"],
  drive: ["drive", "DRIVE_RUNTIME_API_VERSION"],
  s3: ["s3", "S3_RUNTIME_API_VERSION"],
} as const;
for (const [name, contract] of Object.entries(matrix.native_runtime_apis) as [keyof typeof apiBindings, any][]) {
  equal((observedApis as any)[name], contract.version, `${name} runtime API`);
  equal((NATIVE_SDK_VERSION.runtimeApis as any)[apiBindings[name][0]], contract.version, `${name} TypeScript runtime API`);
  equal(Number(rustConstant(rust, apiBindings[name][1])), contract.version, `${name} Rust runtime API`);
}

const observedSchemas = Object.fromEntries(Object.entries(matrix.native_storage_schemas).map(([name, contract]: [string, any]) => [name, storageVersion(contract.source)]));
const schemaBindings = {
  attestation: ["attestation", "ATTESTATION_STORAGE_SCHEMA_VERSION"],
  names: ["names", "NAMES_STORAGE_SCHEMA_VERSION"],
  storage_provider: ["storageProvider", "STORAGE_PROVIDER_STORAGE_SCHEMA_VERSION"],
  drive: ["drive", "DRIVE_STORAGE_SCHEMA_VERSION"],
  s3: ["s3", "S3_STORAGE_SCHEMA_VERSION"],
  transaction_storage: ["transactionStorage", "TRANSACTION_STORAGE_SCHEMA_VERSION"],
  resources: ["resources", "RESOURCES_STORAGE_SCHEMA_VERSION"],
} as const;
for (const [name, contract] of Object.entries(matrix.native_storage_schemas) as [keyof typeof schemaBindings, any][]) {
  equal(observedSchemas[name], contract.version, `${name} storage schema`);
  equal((NATIVE_SDK_VERSION.storageSchemas as any)[schemaBindings[name][0]], contract.version, `${name} TypeScript storage schema`);
  equal(Number(rustConstant(rust, schemaBindings[name][1])), contract.version, `${name} Rust storage schema`);
}
const providerProtocol = Number(read(matrix.service_protocols.storage_provider.source).match(/PROTOCOL_VERSION:\s*u16\s*=\s*(\d+)/)?.[1]);
equal(providerProtocol, matrix.service_protocols.storage_provider.version, "storage provider protocol");
equal(json(matrix.service_protocols.names_label_policy.source).label_policy_version, matrix.service_protocols.names_label_policy.version, "Orbis Names label policy");
equal(NATIVE_SDK_VERSION.serviceProtocols.storageProvider, matrix.service_protocols.storage_provider.version, "TypeScript storage provider protocol");
equal(Number(rustConstant(rust, "STORAGE_PROVIDER_PROTOCOL_VERSION")), matrix.service_protocols.storage_provider.version, "Rust storage provider protocol");
equal(NATIVE_SDK_VERSION.serviceProtocols.namesLabelPolicy, matrix.service_protocols.names_label_policy.version, "TypeScript Orbis Names label policy");
equal(Number(rustConstant(rust, "NAMES_LABEL_POLICY_VERSION")), matrix.service_protocols.names_label_policy.version, "Rust Orbis Names label policy");

equal(vectors.runtime.spec_version, matrix.networks.orbis.spec_version, "vector spec version");
equal(vectors.runtime.transaction_version, matrix.networks.orbis.transaction_version, "vector transaction version");
equal(vectors.runtime.metadata_hash, matrix.networks.orbis.metadata_hash, "vector metadata hash");
for (const vector of vectors.vectors) {
  equal(sha256(vector.path), vector.sha256, `${vector.domain} semantic vector digest`);
  equal(vector.validated_by.includes("origin-rs"), true, `${vector.domain} Rust coverage`);
  equal(vector.validated_by.includes("product-sdk"), true, `${vector.domain} TypeScript coverage`);
}

const adopted = coverage.entries.filter((entry: any) => entry.disposition === "adopt-semantic");
const intentionalChanges = coverage.entries.filter((entry: any) => entry.disposition === "intentional-change");
const retired = coverage.entries.filter((entry: any) => entry.disposition === "retired");
const notApplicable = coverage.entries.filter((entry: any) => entry.disposition === "not-applicable");
const blockContains = (source: string, declaration: string, member: string) => {
  const start = source.search(new RegExp(`(?:(?:pub|export)\\s+)?(?:enum|struct|interface|type)\\s+${declaration}\\b`));
  if (start < 0) return false;
  const next = source.slice(start + 1).search(/\n(?:(?:pub|export)\s+)?(?:enum|struct|interface|type)\s+[A-Za-z]/);
  const block = source.slice(start, next < 0 ? source.length : start + 1 + next);
  if (new RegExp(`(?:^|[^A-Za-z0-9_])${member}(?:[^A-Za-z0-9_]|$)`).test(block)) return true;
  const parent = block.match(/\bextends\s+([A-Za-z][A-Za-z0-9_]*)/)?.[1];
  return parent ? blockContains(source, parent, member) : false;
};
const compatibilityRows = coverage.entries.filter((entry: any) => entry.compatibility_facade !== false);
const migrationRows = coverage.entries.filter((entry: any) => entry.data_migration_input !== false);
const testCaseBlock = (path: string, caseName: string): string => {
  const source = read(path);
  const escaped = caseName.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const start = path.endsWith(".rs")
    ? source.search(new RegExp(`(?:async\\s+)?fn\\s+${escaped}\\s*\\(`))
    : source.search(new RegExp(`test\\(\\s*["']${escaped}["']`));
  if (start < 0) fail(`conformance case does not exist: ${path}::${caseName}`);
  const brace = source.indexOf("{", start);
  if (brace < 0) fail(`conformance case has no executable body: ${path}::${caseName}`);
  let depth = 0;
  for (let index = brace; index < source.length; index++) {
    if (source[index] === "{") depth++;
    else if (source[index] === "}" && --depth === 0) return source.slice(start, index + 1);
  }
  fail(`conformance case is unterminated: ${path}::${caseName}`);
};
const routeIds = new Set(routeContract.routes.map((route: any) => route.id));
const keyOf = (value: any) => JSON.stringify([value.source_id, value.source_symbol_kind, value.source_symbol]);
const adoptedKeys = new Set(adopted.map(keyOf));
const bindingKeys = new Set(m5Bindings.bindings.map((binding: any) => keyOf(binding.key)));
equal(m5Bindings.schema, "cord.m5-sdk-bindings.v1", "M5 binding schema");
equal(m5Bindings.status, coverage.approval_manifest.verdict === "APPROVED"
  ? "authoritative" : "structural-current-approval-pending", "M5 binding status");
equal(m5Bindings.inputs.design_map.sha256, sha256("docs/sdk/contract-to-native-map.json"), "M5 design map digest");
equal(m5Bindings.inputs.route_contract.sha256, sha256("docs/sdk/native-route-contract.json"), "M5 route contract digest");
equal(JSON.stringify(m5Bindings.inputs.approval_manifest), JSON.stringify(coverage.approval_manifest), "M5 architect approval binding");
equal(m5Bindings.counts.design_entries, coverage.entries.length, "M5 design entry count");
equal(m5Bindings.counts.adopted_semantics, adopted.length, "M5 adopted semantic count");
equal(m5Bindings.counts.exact_bindings, adopted.length, "M5 exact binding count");
equal(m5Bindings.counts.distinct_behavior_concepts,
  new Set(m5Bindings.bindings.map((binding: any) => binding.behavior_concept)).size,
  "M5 distinct behavior concept count");
equal(adopted.length, 14, "strict M5 adopted semantic count");
equal(m5Bindings.counts.distinct_behavior_concepts, 7, "strict M5 behavior concept count");
equal(m5Bindings.bindings.filter((binding: any) => binding.binding_kind === "native-route-set").length,
  12, "strict M5 read route binding count");
equal(m5Bindings.bindings.filter((binding: any) => binding.binding_kind === "label-validation-surface").length,
  2, "strict M5 validation surface binding count");
equal(bindingKeys.size, m5Bindings.bindings.length, "M5 binding key uniqueness");
equal(JSON.stringify([...bindingKeys].sort()), JSON.stringify([...adoptedKeys].sort()), "M5 exact adopted key coverage");

const routeHarness = {
  rust: { path: "origin-rs/tests/product_sdk_native.rs", case: "every_authoritative_native_route_constructs_validates_and_dispatches_in_rust" },
  typescript: { path: "product-sdk/tests/conformance/native-route-harness.test.ts", case: "every authoritative native route constructs, validates and selects its exact TS dispatch" },
};
for (const binding of m5Bindings.bindings) {
  const source = adopted.find((entry: any) => keyOf(entry) === keyOf(binding.key));
  if (!source) fail(`M5 binding has no adopted design row: ${keyOf(binding.key)}`);
  // Source identifiers remain immutable provenance; only the native target surface is branded.
  const isLabelValidation = source.source_id === "dotns:contracts/utils/StringUtils.sol"
    && ["isSingleLabel", "isSingleLabelMemory"].includes(source.source_symbol);
  if (source.source_symbol_kind !== "function") {
    fail(`adopted non-function lacks an explicit M5 binding policy: ${keyOf(binding.key)}`);
  }
  equal(binding.behavior_concept, isLabelValidation ? "single-label-validation" : source.source_symbol,
    `M5 behavior concept ${keyOf(binding.key)}`);
  equal(binding.binding_kind, isLabelValidation ? "label-validation-surface" : "native-route-set", `M5 binding kind ${keyOf(binding.key)}`);
  equal(JSON.stringify(binding.navigation_hint), JSON.stringify({
    rust: source.rust_sdk.replaceAll("DotnsQuery", "NamesQuery"),
    typescript: source.typescript_sdk.replace(/^dotns\b/, "names"),
  }), `M5 navigation hint ${keyOf(binding.key)}`);
  if (!Array.isArray(binding.routes) || !binding.routes.length || binding.routes.some((id: string) => !routeIds.has(id))) {
    fail(`M5 binding route set is empty or unknown: ${keyOf(binding.key)}`);
  }
  if (binding.routes.some((id: string) => !id.startsWith("names:"))) fail(`M5 binding escapes Orbis Names: ${keyOf(binding.key)}`);
  equal(JSON.stringify(binding.exercised_by), JSON.stringify(routeHarness), `M5 exact route harness ${keyOf(binding.key)}`);
  if (binding.binding_kind === "label-validation-surface") {
    const surface = binding.validation_surfaces;
    equal(surface.rust.path, "origin-rs/src/product_sdk/domains/names.rs", `M5 label Rust path ${keyOf(binding.key)}`);
    const rustValidation = read(surface.rust.path);
    if (!new RegExp(`pub\\s+struct\\s+${surface.rust.declaration}\\b`).test(rustValidation)
      || !new RegExp(`impl\\s+${surface.rust.declaration}\\s*\\{[\\s\\S]*?pub\\s+fn\\s+${surface.rust.constructor}\\s*\\(`).test(rustValidation)) {
      fail(`missing exact Rust label validation surface ${keyOf(binding.key)}`);
    }
    equal(surface.typescript.path, "product-sdk/src/names.ts", `M5 label TypeScript path ${keyOf(binding.key)}`);
    if (!new RegExp(`export\\s+function\\s+${surface.typescript.function}\\s*\\(`).test(read(surface.typescript.path))) {
      fail(`missing exact TypeScript label validation surface ${keyOf(binding.key)}`);
    }
    equal(surface.vectors.sha256, sha256(surface.vectors.path), `M5 label vector digest ${keyOf(binding.key)}`);
    testCaseBlock(surface.cases.rust.path, surface.cases.rust.case);
    testCaseBlock(surface.cases.typescript.path, surface.cases.typescript.case);
  }
}
equal(compatibilityRows.length, 0, "compatibility facade rows");
equal(migrationRows.length, 0, "data migration rows");

equal(descriptor.runtime.specVersion, matrix.networks.orbis.spec_version, "descriptor spec version");
equal(descriptor.runtime.transactionVersion, matrix.networks.orbis.transaction_version, "descriptor transaction version");
equal(descriptor.runtime.metadataHash, matrix.networks.orbis.metadata_hash, "descriptor metadata hash");
equal(descriptor.networkActivation.state, matrix.networks.orbis.activation_state, "descriptor activation state");
equal(descriptor.networkActivation.productionActivationReady, false, "descriptor production activation gate");
equal(descriptor.fixtureIdentity.genesis_identity, matrix.networks.orbis.candidate_genesis_header_hash, "descriptor candidate genesis");
equal(descriptor.fixtureIdentity.genesis_state_root, matrix.networks.orbis.candidate_genesis_state_root, "descriptor candidate state root");
equal(descriptor.fixtureIdentity.candidate_identity_sha256, matrix.networks.orbis.candidate_genesis_identity_sha256, "descriptor candidate artifact");
equal(descriptor.nativeHostContract.methodCount, NATIVE_HOST_METHODS.length, "descriptor native method count");
equal(routeContract.schema, "cord.native-route-contract.v1", "route contract schema");
equal(routeContract.route_count, 143, "route contract count");
equal(routeContract.network.metadata_hash, matrix.networks.orbis.metadata_hash, "route contract metadata hash");
equal(routeContract.network.activation_state, matrix.networks.orbis.activation_state, "route contract activation state");
equal(routeContract.network.production_activation_ready, false, "route contract production gate");
equal(routeContract.signature_schema_basis.runtime_metadata,
  "current RFC-78 metadata hash is reproduced, but no decoded metadata blob is available; runtime argument signatures are not asserted from decoded metadata",
  "route contract decoded metadata boundary");
equal(routeContract.routes.length, NATIVE_HOST_METHODS.length, "route projection count");
const projectedRoutes = routeContract.routes.map((route: any) => ({ capability: route.capability, method: route.method, finality: route.finality, payloadFields: route.parameters.map(({ name }: any) => name) }));
equal(JSON.stringify(projectedRoutes), JSON.stringify(NATIVE_HOST_METHODS), "authoritative TypeScript route projection");
equal(JSON.stringify(Object.keys(NATIVE_RUNTIME_ROUTE_REGISTRY)), JSON.stringify(routeContract.routes.map((route: any) => route.id)), "direct TypeScript callable registry");
const runtimeTable = read("origin/orbis/runtime/src/lib.rs");
const routeGaps: string[] = [];
const palletCallItems = new Map<string, Map<string, number>>();
const writeRoutes = routeContract.routes.filter((route: any) => route.runtime.kind === "pallet-call");
equal(writeRoutes.length, 84, "authoritative pallet call route count");
equal(new Set(writeRoutes.map((route: any) => `${route.runtime.source}#${route.runtime.target}`)).size, 84, "distinct pallet call item count");
const sdkHostRoutes = routeContract.routes.filter((route: any) => route.runtime.kind === "sdk-host-operation");
equal(sdkHostRoutes.length, 1, "authoritative SDK host operation count");
for (const route of routeContract.routes) {
  const parameters = route.parameters.map(({ name }: any) => name);
  if (Object.keys(route.sample_payload).join() !== parameters.join()) routeGaps.push(`${route.id}:sample parameter order`);
  const rustSdkFunction = route.rust.binding_kind === "sdk-function";
  if (rustSdkFunction) {
    const rustProductSource = read(route.rust.source);
    if (!new RegExp(`pub\\s+async\\s+fn\\s+${route.rust.declaration}\\s*\\(`).test(rustProductSource)) {
      routeGaps.push(`${route.id}:Rust SDK function ${route.rust.declaration}`);
    }
  } else if (!blockContains(rustSurface, route.rust.declaration, route.rust.variant)) {
    routeGaps.push(`${route.id}:Rust ${route.rust.declaration}::${route.rust.variant}`);
  }
  if (typeof (NATIVE_RUNTIME_ROUTE_REGISTRY as any)[route.id] !== "function") routeGaps.push(`${route.id}:direct TypeScript callable`);
  const coreSchema = methodPayloadSchema(route.capability, route.method) as any;
  const sourceParameters = Object.entries(coreSchema.properties).map(([name, schema]) => ({ name, schema }));
  if (JSON.stringify(route.parameters) !== JSON.stringify(sourceParameters)
    || JSON.stringify(route.sample_payload) !== JSON.stringify(canonicalMethodPayload(route.capability, route.method))
    || !Array.isArray(route.canonical_arguments)) routeGaps.push(`${route.id}:core-schema/canonical-factory binding`);
  const runtimeSource = read(route.runtime.source);
  const target = route.runtime.target.replace(/[.*+?^${}()|[\\]\\\\]/g, "\\\\$&");
  if (route.runtime.kind === "pallet-call") {
    let calls = palletCallItems.get(route.runtime.source);
    if (!calls) {
      calls = parsePalletCallItems(runtimeSource, route.runtime.source);
      palletCallItems.set(route.runtime.source, calls);
    }
    if (calls.get(route.runtime.target) !== route.runtime.call_index) routeGaps.push(`${route.id}:dispatch index ${route.runtime.pallet_index}/${route.runtime.call_index}`);
    if (!new RegExp(`\\b${route.runtime.pallet}:\\s+[^=]+\\s*=\\s*${route.runtime.pallet_index},`).test(runtimeTable)) routeGaps.push(`${route.id}:pallet index ${route.runtime.pallet_index}`);
  } else if (route.runtime.kind === "runtime-api") {
    if (!new RegExp(`(?:pub\\s+)?trait\\s+[A-Za-z0-9_]+[\\s\\S]*?fn\\s+${target}\\s*\\(`).test(runtimeSource)) routeGaps.push(`${route.id}:runtime API declaration`);
    if (!new RegExp(`fn\\s+${target}\\s*\\(`).test(read(route.runtime.implementation_source))) routeGaps.push(`${route.id}:runtime API implementation`);
  } else if (route.runtime.kind === "sdk-host-operation") {
    if (!rustSdkFunction || route.rust.source !== route.runtime.source || route.rust.declaration !== route.runtime.target) {
      routeGaps.push(`${route.id}:SDK host operation binding`);
    }
    if (!new RegExp(`pub\\s+async\\s+fn\\s+${target}\\s*\\(`).test(runtimeSource)) {
      routeGaps.push(`${route.id}:SDK host operation implementation`);
    }
    for (const forbiddenField of ["pallet", "pallet_index", "call_index", "runtime_api_version", "implementation_source"]) {
      if (Object.hasOwn(route.runtime, forbiddenField)) routeGaps.push(`${route.id}:SDK host operation exposes ${forbiddenField}`);
    }
  } else {
    routeGaps.push(`${route.id}:unsupported runtime binding kind ${route.runtime.kind}`);
  }
}
if (routeGaps.length) fail(`authoritative native route contract gaps (${routeGaps.length}): ${routeGaps.slice(0, 20).join("; ")}`);
const rustHarness = testCaseBlock("origin-rs/tests/product_sdk_native.rs", "every_authoritative_native_route_constructs_validates_and_dispatches_in_rust");
const tsHarness = testCaseBlock("product-sdk/tests/conformance/native-route-harness.test.ts", "every authoritative native route constructs, validates and selects its exact TS dispatch");
if (!rustHarness.includes("host.execute(request)") || !tsHarness.includes("await host.execute(request)")) fail("native route harnesses do not execute requests");
if (!rustHarness.includes("instantiate_native_route(route)") || !rustHarness.includes("validate_and_prepare()")) fail("Rust route harness does not exercise canonical typed factories");
if (!tsHarness.includes("NATIVE_RUNTIME_ROUTE_REGISTRY[scope]")) fail("TypeScript route harness does not invoke the direct callable registry");
const rustHarnessRun = spawnSync("cargo", [
  "test", "-p", "origin-rs", "--test", "product_sdk_native",
  "every_authoritative_native_route_constructs_validates_and_dispatches_in_rust",
  "--locked", "--", "--exact",
], { cwd: root, encoding: "utf8" });
if (rustHarnessRun.status !== 0) {
  fail(`Rust authoritative route harness failed:\n${rustHarnessRun.stdout}${rustHarnessRun.stderr}`);
}
const tsHarnessRun = spawnSync(process.execPath, [
  "--experimental-strip-types", "--test",
  "product-sdk/tests/conformance/native-route-harness.test.ts",
], { cwd: root, encoding: "utf8" });
if (tsHarnessRun.status !== 0) {
  fail(`TypeScript authoritative route harness failed:\n${tsHarnessRun.stdout}${tsHarnessRun.stderr}`);
}
const forbidden = /(?:^|[_-])(raw[_-]?scale|scale[_-]?(?:bytes|payload)|abi|contract[_-]?address|revive|pallet[_-]?index|call[_-]?index)(?:$|[_-])/i;
const exposedTokens = NATIVE_HOST_METHODS.flatMap(({ capability, method, payloadFields }) => [capability, method, ...payloadFields]);
const forbiddenTokens = exposedTokens.filter((token) => forbidden.test(token));
equal(forbiddenTokens.length, 0, "raw SCALE/Revive/ABI/index reference tokens");

const report = {
  schema: "cord.sdk-native-coverage.report.v1",
  status: coverage.approval_manifest.verdict === "APPROVED" ? "pass" : "structural-pass-approval-pending",
  approval: coverage.approval_manifest,
  release: matrix.release,
  clean_break: true,
  runtime: {
    origin: { spec_version: matrix.networks.origin.spec_version, transaction_version: matrix.networks.origin.transaction_version },
    orbis: { para_id: matrix.networks.orbis.para_id, spec_version: matrix.networks.orbis.spec_version, transaction_version: matrix.networks.orbis.transaction_version, metadata_hash: matrix.networks.orbis.metadata_hash },
  },
  sdk: { rust: matrix.packages.rust.version, typescript: matrix.packages.typescript.version, contract_version: matrix.contract_version },
  runtime_apis: Object.fromEntries(Object.entries(matrix.native_runtime_apis).map(([name, value]: [string, any]) => [name, value.version])),
  storage_schemas: Object.fromEntries(Object.entries(matrix.native_storage_schemas).map(([name, value]: [string, any]) => [name, value.version])),
  semantic_vectors: vectors.vectors.map((vector: any) => ({ domain: vector.domain, sha256: vector.sha256 })),
  coverage: {
    design_entries: coverage.entries.length,
    adopted_semantics: adopted.length,
    exact_m5_bindings: m5Bindings.bindings.length,
    distinct_behavior_concepts: m5Bindings.counts.distinct_behavior_concepts,
    intentional_change_entries: intentionalChanges.length,
    retired_entries: retired.length,
    not_applicable_entries: notApplicable.length,
    binding_kinds: Object.fromEntries([...new Set(m5Bindings.bindings.map((binding: any) => binding.binding_kind))]
      .sort().map((kind) => [kind, m5Bindings.bindings.filter((binding: any) => binding.binding_kind === kind).length])),
    executable_route_cases: { distinct_routes: routeContract.route_count, rust: routeContract.route_count, typescript: routeContract.route_count },
    compatibility_facade_rows: compatibilityRows.length,
    data_migration_rows: migrationRows.length,
  },
  descriptor: { kind: descriptor.kind, native_method_count: NATIVE_HOST_METHODS.length, authoritative_route_contract: "docs/sdk/native-route-contract.json", runtime_dispatch_or_api_bound_routes: routeContract.route_count, metadata_reconciliation: "exact current metadata hash plus source-derived pallet/call indices and runtime API implementation tables; decoded current metadata blob unavailable, so decoded runtime signatures are not asserted" },
  reference_surface: { raw_scale: false, migrated_domain_revive: false, contract_abi: false, pallet_or_call_indices: false },
  inputs: {
    version_matrix_sha256: sha256("docs/sdk/native-version-matrix.json"),
    coverage_map_sha256: sha256("docs/sdk/contract-to-native-map.json"),
    m5_sdk_bindings_sha256: sha256("docs/sdk/m5-sdk-bindings.json"),
    route_contract_sha256: sha256("docs/sdk/native-route-contract.json"),
    rust_route_harness_sha256: sha256("origin-rs/tests/product_sdk_native.rs"),
    typescript_route_harness_sha256: sha256("product-sdk/tests/conformance/native-route-harness.test.ts"),
    descriptor_sha256: sha256("product-sdk/packages/descriptors/generated/orbis-descriptor.json"),
  },
};
const reportPath = resolve(root, "docs/evidence/verification/p5/sdk-native-coverage.report.json");
const serialized = `${JSON.stringify(report, null, 2)}\n`;
if (process.argv.includes("--write")) writeFileSync(reportPath, serialized);
else if (readFileSync(reportPath, "utf8") !== serialized) fail("native SDK coverage report drift; run npm run update:sdk-freeze");
process.stdout.write(`PASS native SDK mappings: runtime=${matrix.networks.orbis.spec_version}/${matrix.networks.orbis.transaction_version} design=${coverage.entries.length} adopted=${adopted.length} exact_m5_bindings=${m5Bindings.bindings.length} intentional_change=${intentionalChanges.length} executable_routes=${routeContract.route_count} Rust=${routeContract.route_count} TS=${routeContract.route_count}\n`);
