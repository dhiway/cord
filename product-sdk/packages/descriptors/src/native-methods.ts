import { NATIVE_ROUTE_CONTRACT } from "../generated/native-route-contract.ts";

export type NativeHostFinality = "finalized" | "submit-and-finalize";
export interface NativeHostMethodContract {
  readonly capability: "identity" | "attestation" | "names" | "storage" | "content" | "assets" | "transaction";
  readonly method: string;
  readonly finality: NativeHostFinality;
  readonly payloadFields: readonly string[];
}

/** Generated projection of the authoritative checked-in native route contract. */
export const NATIVE_HOST_METHODS = NATIVE_ROUTE_CONTRACT.routes.map((route) => ({
  capability: route.capability,
  method: route.method,
  finality: route.finality,
  payloadFields: route.parameters.map(({ name }) => name),
})) satisfies readonly NativeHostMethodContract[];

const identities = NATIVE_HOST_METHODS.map(({ capability, method }) => `${capability}:${method}`);
if (new Set(identities).size !== identities.length) throw new Error("duplicate native host method identity");
