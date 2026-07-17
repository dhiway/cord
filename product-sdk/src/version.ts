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

/** Canonical first-supported clean-break native SDK/runtime contract. */
export const NATIVE_SDK_VERSION = {
  contractVersion: 1,
  release: "origin-orbis-native-v1",
  sdkRelease: "0.9.9",
  origin: {
    specVersion: 9901,
    transactionVersion: 2,
    activationState: "candidate-pending",
    productionActivationReady: false,
  },
  orbis: {
    paraId: 1006,
    specVersion: 29,
    transactionVersion: 8,
    metadataHash: "0x50c8958f0171889a4b01093a5dd5272faf8802018f24a4ac22b231d37b2adc45",
    candidateGenesisHeaderHash: "0x2584c9d420dc8160b85deaf958d776886366d5beecc2ee7b1236293d20ac70fc",
    candidateGenesisStateRoot: "0xe778ef91e9e1419f77a17a687245fc903335ecdb8dd3cf450df86637d529a53c",
    candidateGenesisIdentitySha256: "e0cfdc509e90b58c7c36a9cc522eab9013f281c076c100ec3f09fc9e24c074de",
    activationState: "candidate-pending",
    productionActivationReady: false,
  },
  runtimeApis: {
    identityPersonhood: 1,
    attestation: 1,
    names: 1,
    storageProvider: 11,
    drive: 2,
    s3: 3,
  },
  storageSchemas: {
    attestation: 1,
    names: 1,
    storageProvider: 2,
    drive: 1,
    s3: 2,
    resources: 1,
  },
  serviceProtocols: {
    namesLabelPolicy: 1,
    storageProvider: 6,
  },
  cleanBreak: true,
  rawScaleProductApi: false,
  migratedDomainRevive: false,
} as const;

export type NativeSdkVersion = typeof NATIVE_SDK_VERSION;
