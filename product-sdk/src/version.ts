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
    candidateGenesisHeaderHash: "0x657de1aa28685cfa4c66e9f3186c586f3d85db02724bcbdd1a620e0a5cc4e173",
    candidateGenesisStateRoot: "0x3455b0234339e9f97fbdf926721267226782dffaffd382f3f1e9f472099615e1",
    candidateGenesisIdentitySha256: "2a7a8d5b8fce4fc2dd15729c3e414d6d68e82ef9a21e12c382b5676d3530ed68",
    activationState: "candidate-pending",
    productionActivationReady: false,
  },
  runtimeApis: {
    identityPersonhood: 1,
    attestation: 1,
    names: 1,
    storageProvider: 4,
    drive: 1,
    s3: 1,
  },
  storageSchemas: {
    attestation: 1,
    names: 1,
    storageProvider: 5,
    drive: 1,
    s3: 1,
    transactionStorage: 8,
    resources: 1,
  },
  serviceProtocols: {
    namesLabelPolicy: 1,
    storageProvider: 4,
  },
  cleanBreak: true,
  rawScaleProductApi: false,
  migratedDomainRevive: false,
} as const;

export type NativeSdkVersion = typeof NATIVE_SDK_VERSION;
