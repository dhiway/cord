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
    candidateGenesisHeaderHash: "0x98cd55908bb19c4006abf021ccb7be2ece52c8edca8149f739145836ccbfce03",
    candidateGenesisStateRoot: "0x1d1ebb75fa374a0dac0a934663c724abddb67b7f41b729431e953655783e0390",
    candidateGenesisIdentitySha256: "d1c9f8ba39a4af9bf618bcde2e832396d6a60addd0fad0cb24797866e8edc4ec",
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
