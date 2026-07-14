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
    metadataHash: "0xa11fc57ceabd72676b4f1f6dec860de0c8f52f9d2e9496ea36366d1ba47cd391",
    candidateGenesisHeaderHash: "0x066f97db4ab6a5e5d44ee66c3b469f82d178817650fa6f52634ea2ebead6c6e3",
    candidateGenesisStateRoot: "0x55e81b8c3aada6227214d467cefad014198e84d4e3fa15c8021e31dcc40fe0dc",
    candidateGenesisIdentitySha256: "8b4986d5d6c7b4e92a29b53424bedb75303dbd80c77137f996d40ae334a4b59f",
    activationState: "candidate-pending",
    productionActivationReady: false,
  },
  runtimeApis: {
    identityPersonhood: 1,
    attestation: 1,
    dotns: 1,
    storageProvider: 4,
    drive: 1,
    s3: 1,
  },
  storageSchemas: {
    attestation: 1,
    dotns: 1,
    storageProvider: 5,
    drive: 1,
    s3: 1,
    transactionStorage: 8,
    resources: 1,
  },
  serviceProtocols: {
    dotnsLabelPolicy: 1,
    storageProvider: 4,
  },
  cleanBreak: true,
  rawScaleProductApi: false,
  migratedDomainRevive: false,
} as const;

export type NativeSdkVersion = typeof NATIVE_SDK_VERSION;
