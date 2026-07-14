import {
  NativeDomainError,
  type JsonObject,
  type JsonValue,
  type NativeDomain,
} from "./errors.ts";

export type NativeCapability = "identity" | "attestation" | "dotns" | "storage" | "transaction";
export type HostFinality = "finalized" | "submit-and-finalize";
export type ConsentScope<Capability extends string = string, Method extends string = string> =
  `${Capability}:${Method}`;

export interface NetworkBinding {
  readonly genesis_hash: string;
  readonly spec_version: number;
  readonly transaction_version: number;
  readonly metadata_hash: string;
  readonly descriptor_contract_sha256: string;
  readonly chain_spec_source_sha256: string;
  readonly activation_state: "candidate-pending" | "production-approved";
  readonly production_activation_ready: boolean;
  readonly access_mode: "candidate" | "production";
}

export interface ActiveConsent {
  /** Scopes granted by the application host. Requests disclose only their exact required scope. */
  readonly scopes: readonly ConsentScope[];
  readonly expires_at: number;
  readonly nonce: string;
}

export interface RequestContext {
  readonly request_id: string;
  readonly application_id: string;
  readonly network: NetworkBinding;
  readonly consent: ActiveConsent;
}

/** Wire-compatible with the existing product-sdk HostRequest v1 envelope. */
export interface HostRequest<
  Capability extends NativeCapability = NativeCapability,
  Method extends string = string,
  Payload extends JsonObject = JsonObject,
  Finality extends HostFinality = HostFinality,
> {
  readonly version: 1;
  readonly request_id: string;
  readonly application_id: string;
  readonly capability: Capability;
  readonly method: Method;
  readonly network: NetworkBinding;
  readonly finality: Finality;
  readonly payload: Payload;
  readonly consent: {
    readonly scope: ConsentScope[];
    readonly expires_at: number;
    readonly nonce: string;
  };
}

const forbiddenPayloadKeys = new Set([
  "scale",
  "rawscale",
  "scalebytes",
  "abi",
  "contractabi",
  "contractaddress",
  "deploymentaddress",
]);

function normalizedKey(key: string): string {
  return key.normalize("NFKC").toLowerCase().replace(/[^a-z0-9]/g, "");
}

function assertNativePayload(
  domain: NativeDomain,
  operation: string,
  value: JsonValue,
  path = "payload",
): void {
  if (
    typeof value === "string" &&
    /(?:raw[\s_-]*scale|contract[\s_-]*(?:abi|address)|solidity|evm[\s_-]*(?:address|selector)|h160)/i.test(value)
  ) {
    throw new NativeDomainError(domain, operation, "invalid_input", `non-native value at ${path}`);
  }
  if (Array.isArray(value)) {
    value.forEach((item, index) => assertNativePayload(domain, operation, item, `${path}[${index}]`));
    return;
  }
  if (value && typeof value === "object") {
    for (const [key, child] of Object.entries(value)) {
      const normalized = normalizedKey(key);
      if (
        forbiddenPayloadKeys.has(normalized) ||
        normalized.includes("scale") ||
        normalized === "abi" || normalized.startsWith("abi") || normalized.endsWith("abi")
          || normalized.includes("contractabi") ||
        normalized.includes("selector") ||
        normalized.includes("solidity") ||
        normalized.includes("contract") ||
        normalized.includes("h160") ||
        normalized.includes("evm")
      ) {
        throw new NativeDomainError(domain, operation, "invalid_input", `non-native field ${path}.${key}`);
      }
      assertNativePayload(domain, operation, child, `${path}.${key}`);
    }
  }
}

function request<
  Capability extends NativeCapability,
  Method extends string,
  Payload extends JsonObject,
  Finality extends HostFinality,
>(
  domain: NativeDomain,
  context: RequestContext,
  capability: Capability,
  method: Method,
  finality: Finality,
  payload: Payload,
): HostRequest<Capability, Method, Payload, Finality> {
  const operation = `${capability}.${method}`;
  const requiredScope = `${capability}:${method}` as ConsentScope;
  if (!context.consent.scopes.includes(requiredScope)) {
    throw new NativeDomainError(
      domain,
      operation,
      "not_authorized",
      `active consent does not include ${requiredScope}`,
    );
  }
  if (context.request_id.length < 16 || context.request_id.length > 128) {
    throw new NativeDomainError(domain, operation, "invalid_input", "request_id must contain 16-128 characters");
  }
  if (!context.application_id || context.application_id.length > 128) {
    throw new NativeDomainError(domain, operation, "invalid_input", "application_id must contain 1-128 characters");
  }
  if (!Number.isSafeInteger(context.consent.expires_at) || context.consent.expires_at < 1) {
    throw new NativeDomainError(domain, operation, "invalid_input", "consent expiry must be a positive integer");
  }
  if (context.consent.nonce.length < 16 || context.consent.nonce.length > 128) {
    throw new NativeDomainError(domain, operation, "invalid_input", "consent nonce must contain 16-128 characters");
  }
  assertNativePayload(domain, operation, payload);
  return {
    version: 1,
    request_id: context.request_id,
    application_id: context.application_id,
    capability,
    method,
    network: { ...context.network },
    finality,
    payload,
    consent: {
      scope: [requiredScope],
      expires_at: context.consent.expires_at,
      nonce: context.consent.nonce,
    },
  };
}

export function finalizedRead<
  Capability extends NativeCapability,
  Method extends string,
  Payload extends JsonObject,
>(
  domain: NativeDomain,
  context: RequestContext,
  capability: Capability,
  method: Method,
  payload: Payload,
): HostRequest<Capability, Method, Payload, "finalized"> {
  return request(domain, context, capability, method, "finalized", payload);
}

export function submitAndFinalize<
  Capability extends NativeCapability,
  Method extends string,
  Payload extends JsonObject,
>(
  domain: NativeDomain,
  context: RequestContext,
  capability: Capability,
  method: Method,
  payload: Payload,
): HostRequest<Capability, Method, Payload, "submit-and-finalize"> {
  return request(domain, context, capability, method, "submit-and-finalize", payload);
}
