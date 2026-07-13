export interface OrbisDescriptorContract {
  contractVersion: 1;
  kind: "papi-bootstrap-descriptor-contract";
  runtime: {
    name: "orbis";
    paraId: 1006;
    specVersion: 29;
    transactionVersion: 8;
    metadataHash: `0x${string}`;
  };
  fixtureIdentity: {
    status: "unfinalized-p0-fixture-not-production-genesis";
    genesis_identity: `0x${string}`;
    chain_spec_source: string;
    chain_spec_source_sha256: string;
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
      capability: "identity" | "attestation" | "dotns" | "storage" | "content" | "assets" | "transaction";
      method: string;
      finality: "finalized" | "submit-and-finalize";
      payloadFields: readonly string[];
    }[];
  };
  productionPapiDescriptorGenerated: false;
}
