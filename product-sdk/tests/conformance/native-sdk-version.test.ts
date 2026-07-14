import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import test from "node:test";
import { NATIVE_SDK_VERSION } from "../../src/version.ts";

const repo = resolve(import.meta.dirname, "../../..");
const matrix = JSON.parse(readFileSync(resolve(repo, "docs/sdk/native-version-matrix.json"), "utf8"));

test("TypeScript freezes the canonical Origin/Orbis native version matrix", () => {
  assert.equal(NATIVE_SDK_VERSION.contractVersion, matrix.contract_version);
  assert.equal(NATIVE_SDK_VERSION.sdkRelease, matrix.sdk_release);
  assert.deepEqual(NATIVE_SDK_VERSION.origin, {
    specVersion: matrix.networks.origin.spec_version,
    transactionVersion: matrix.networks.origin.transaction_version,
    activationState: matrix.networks.origin.activation_state,
    productionActivationReady: matrix.networks.origin.production_activation_ready,
  });
  assert.deepEqual(NATIVE_SDK_VERSION.orbis, {
    paraId: matrix.networks.orbis.para_id,
    specVersion: matrix.networks.orbis.spec_version,
    transactionVersion: matrix.networks.orbis.transaction_version,
    metadataHash: matrix.networks.orbis.metadata_hash,
    candidateGenesisHeaderHash: matrix.networks.orbis.candidate_genesis_header_hash,
    candidateGenesisStateRoot: matrix.networks.orbis.candidate_genesis_state_root,
    candidateGenesisIdentitySha256: matrix.networks.orbis.candidate_genesis_identity_sha256,
    activationState: matrix.networks.orbis.activation_state,
    productionActivationReady: matrix.networks.orbis.production_activation_ready,
  });
  assert.equal(matrix.clean_break.new_network, true);
  assert.equal(matrix.clean_break.backward_compatibility, false);
  assert.equal(matrix.clean_break.data_migration, false);
  assert.equal(matrix.clean_break.legacy_client, false);
  assert.equal(matrix.clean_break.contract_compatibility_facade, false);
  assert.equal(matrix.surface_policy.raw_scale_product_api, false);
  assert.equal(matrix.surface_policy.migrated_domain_revive, false);
});
