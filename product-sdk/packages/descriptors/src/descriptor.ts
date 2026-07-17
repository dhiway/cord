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

export interface OrbisDescriptorContract {
  contractVersion: 1;
  kind: "cord-native-host-contract-manifest";
  release: "origin-orbis-native-v1";
  firstSupportedNativeSdk: false;
  runtime: {
    name: "orbis";
    paraId: 1006;
    specVersion: 29;
    transactionVersion: 8;
    metadataHash: `0x${string}`;
  };
  currentSourceRuntime: {
    name: "commons";
    source: "origin/orbis/runtime/src/lib.rs";
    metadataRecord: "origin/orbis/runtime/vectors/transaction-policy-v8/metadata-hash.json";
    specVersion: 33;
    transactionVersion: 8;
    metadataHash: `0x${string}`;
    metadataBoundNativeSdk: false;
  };
  fixtureIdentity: {
    status: "deterministic-clean-break-candidate-not-production-approved";
    genesis_identity: `0x${string}`;
    chain_spec_source: string;
    chain_spec_source_sha256: string;
  };
  networkActivation: {
    state: "candidate-pending" | "production-approved";
    productionActivationReady: boolean;
    source: string;
  };
  ratificationPayloadSha256: string;
  sources: Record<string, { path: string; sha256: string }>;
  signedExtensionSurfaces: Record<string, readonly string[]>;
  nativeHostContract: {
    version: 1;
    methodCount: number;
    pageLimit: 100;
    payloadValidation: "closed-shape-plus-core-native-types-v1";
    methods: readonly {
      capability: "identity" | "attestation" | "names" | "storage" | "content" | "assets" | "transaction";
      method: string;
      finality: "finalized" | "submit-and-finalize";
      payloadFields: readonly string[];
    }[];
  };
  descriptorProvenance: {
    runtimeMetadataBinding: string;
    papiDescriptor: string;
    methodInventory: string;
    driftValidation: string;
  };
  papiAvailability: {
    runtimeMetadataCurrent: false;
    sdkAdmission: false;
    reason: string;
  };
  productionPapiDescriptorGenerated: false;
}
