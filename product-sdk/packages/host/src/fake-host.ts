import { createHash } from "node:crypto";
import {
  assertMethodFinality,
  assertMethodPayload,
  assertRuntimeIdentity,
  ProductSdkError,
  validateLifecycle,
  type JsonObject,
  type JsonValue,
} from "../../core/src/contract.ts";
import { NATIVE_HOST_METHODS } from "../../descriptors/src/native-methods.ts";

export interface HostRequest {
  version: 1; request_id: string; application_id: string; capability: string; method: string;
  network: { genesis_hash: string; spec_version: number; transaction_version: number; metadata_hash: string; descriptor_contract_sha256: string; chain_spec_source_sha256: string; activation_state: "candidate-pending" | "production-approved"; production_activation_ready: boolean; access_mode: "candidate" | "production" };
  finality: "finalized" | "submit-and-finalize"; payload: JsonObject;
  consent: { scope: string[]; expires_at: number; nonce: string };
}
export interface SignedRequest { request: HostRequest; signature: string }
export interface HostTransportResult {
  /** Finalized state used for a read, or the block finalizing a submitted extrinsic. */
  finalizedHash: string;
  /** Decoded native runtime-API/storage response. The host never returns runtime-encoded bytes. */
  response?: JsonValue;
  /** Finalized extrinsic hash when the submit adapter can provide it. */
  extrinsicHash?: string;
  /** Native lifecycle-v1 envelope when the submit adapter can provide complete evidence. */
  lifecycle?: JsonObject;
}
export type HostRoute = (signed: SignedRequest, signal: AbortSignal) => Promise<HostTransportResult>;
export interface HostDependencies {
  now?: () => number;
  signer?: (request: HostRequest) => Promise<string>;
  /** Backward-compatible fallback used for both routes when a dedicated route is absent. */
  transport?: HostRoute;
  /** Finalized runtime-API/storage-query route. */
  finalizedRead?: HostRoute;
  /** Metadata-driven submit route that resolves only after finalization. */
  submitAndFinalize?: HostRoute;
  onTerminal?: (requestId: string, outcome: string) => void;
}
const CAPABILITIES = new Set(["identity", "attestation", "dotns", "storage", "content", "assets", "transaction"]);
const METHOD_SCOPES = new Set(NATIVE_HOST_METHODS.map(({ capability, method }) => `${capability}:${method}`));

export class FakeHost {
	private readonly permissions = new Map<string, Set<string>>(); private readonly revoked = new Set<string>();
	private readonly consents = new Map<string, { applicationId: string; scope: string[]; expiresAt: number; claimedBy?: string }>();
	private readonly revokedConsents = new Set<string>();
	private readonly consumedNonces = new Set<string>(); private readonly preCancelled = new Set<string>();
  private readonly inflight = new Map<string, AbortController>(); private readonly terminal = new Set<string>(); private readonly seen = new Set<string>();
  private readonly now: () => number; private readonly signer: NonNullable<HostDependencies["signer"]>;
  private readonly finalizedRead: HostRoute; private readonly submitAndFinalize: HostRoute;
  private readonly onTerminal: NonNullable<HostDependencies["onTerminal"]>;
  constructor(deps: HostDependencies = {}) {
    this.now = deps.now ?? (() => 1_000);
    this.signer = deps.signer ?? (async (request) => createHash("sha256").update(JSON.stringify(request)).digest("hex"));
    const fallback = deps.transport ?? (async () => ({ finalizedHash: `0x${"ab".repeat(32)}` }));
    this.finalizedRead = deps.finalizedRead ?? fallback;
    this.submitAndFinalize = deps.submitAndFinalize ?? fallback;
    this.onTerminal = deps.onTerminal ?? (() => {});
  }
	grant(applicationId: string, methodScopes: string[]): void {
		if (!applicationId || !methodScopes.length || methodScopes.some((scope) => !METHOD_SCOPES.has(scope)))
			throw new ProductSdkError("invalid_input", "host permissions must be exact capability:method scopes");
		this.permissions.set(applicationId, new Set(methodScopes));
	}
	issueConsent(applicationId: string, consent: HostRequest["consent"]): void {
		if (!applicationId || !Array.isArray(consent.scope) || !consent.scope.length
			|| consent.scope.some((scope) => typeof scope !== "string" || !METHOD_SCOPES.has(scope))
			|| new Set(consent.scope).size !== consent.scope.length
			|| !Number.isInteger(consent.expires_at) || consent.expires_at < 1
			|| typeof consent.nonce !== "string" || consent.nonce.length < 16 || consent.nonce.length > 128)
			throw new ProductSdkError("invalid_input", "invalid host-owned consent");
		if (this.consents.has(consent.nonce) || this.consumedNonces.has(consent.nonce))
			throw new ProductSdkError("conflict", "consent nonce already issued or consumed");
		this.consents.set(consent.nonce, {
			applicationId,
			scope: [...consent.scope],
			expiresAt: consent.expires_at,
			claimedBy: undefined,
		});
	}
	revokeConsent(nonce: string): void { this.revokedConsents.add(nonce); }
	revoke(applicationId: string): void { this.revoked.add(applicationId); }
  cancel(requestId: string): void {
    const controller = this.inflight.get(requestId);
    if (controller) { if (!controller.signal.aborted) controller.abort(); }
    else this.preCancelled.add(requestId);
  }
  private finish(id: string, outcome: string): void { if (!this.terminal.has(id)) { this.terminal.add(id); this.onTerminal(id, outcome); } }
  async execute(request: HostRequest): Promise<HostTransportResult> {
    let outcome = "runtime_rejected",ownsId=false;
    try {
      validateHostRequest(request);
      if(this.seen.has(request.request_id))throw new ProductSdkError("conflict","duplicate or in-flight request_id");this.seen.add(request.request_id);ownsId=true;
      if (this.preCancelled.has(request.request_id)) throw new ProductSdkError("cancelled", "request cancelled");
      if (this.revoked.has(request.application_id)) throw new ProductSdkError("permission_revoked", "permission revoked before signing");
			const methodScope = `${request.capability}:${request.method}`;
			if (!CAPABILITIES.has(request.capability) || !this.permissions.get(request.application_id)?.has(methodScope)) throw new ProductSdkError("permission_denied", "method not granted by host");
			if (this.consumedNonces.has(request.consent.nonce)) throw new ProductSdkError("replay", "consent nonce already used");
			const consent = this.consents.get(request.consent.nonce);
			if (!consent || consent.applicationId !== request.application_id) throw new ProductSdkError("permission_denied", "host-owned consent is missing");
			if (this.revokedConsents.has(request.consent.nonce)) throw new ProductSdkError("permission_revoked", "host-owned consent was revoked");
			if (consent.expiresAt !== request.consent.expires_at
				|| consent.scope.length !== request.consent.scope.length
				|| consent.scope.some((scope, index) => scope !== request.consent.scope[index])
				|| !consent.scope.includes(methodScope)) throw new ProductSdkError("permission_denied", "request consent does not match the host-owned record");
			if (consent.expiresAt <= this.now()) throw new ProductSdkError("consent_expired", "consent expired");
			if (consent.claimedBy !== undefined) throw new ProductSdkError("replay", "consent nonce is in flight");
			consent.claimedBy = request.request_id;
      const controller = new AbortController(); this.inflight.set(request.request_id, controller);
      const aborted = new Promise<never>((_, reject) => controller.signal.addEventListener("abort", () => reject(new ProductSdkError("cancelled", "in-flight request cancelled")), { once: true }));
      const signature = await Promise.race([this.signer(request), aborted]);
      if (this.revoked.has(request.application_id) || !this.permissions.get(request.application_id)?.has(methodScope))
        throw new ProductSdkError("permission_revoked", "application grant was revoked during signing");
      if (this.consumedNonces.has(request.consent.nonce)) throw new ProductSdkError("replay", "consent nonce already used");
      const currentConsent = this.consents.get(request.consent.nonce);
      if (this.revokedConsents.has(request.consent.nonce))
        throw new ProductSdkError("permission_revoked", "host-owned consent was revoked during signing");
      if (!currentConsent || currentConsent.applicationId !== request.application_id
        || currentConsent.expiresAt !== request.consent.expires_at
        || currentConsent.claimedBy !== request.request_id
        || currentConsent.scope.length !== request.consent.scope.length
        || currentConsent.scope.some((scope, index) => scope !== request.consent.scope[index])
        || !currentConsent.scope.includes(methodScope))
        throw new ProductSdkError("permission_revoked", "host-owned consent changed during signing");
      if (currentConsent.expiresAt <= this.now()) throw new ProductSdkError("consent_expired", "consent expired during signing");
      this.consents.delete(request.consent.nonce); this.consumedNonces.add(request.consent.nonce);
      const route = request.finality === "finalized" ? this.finalizedRead : this.submitAndFinalize;
      const result = await Promise.race([route({ request, signature }, controller.signal), aborted]);
      if (!result || typeof result.finalizedHash !== "string" || !result.finalizedHash)
        throw new ProductSdkError("runtime_rejected", "host route omitted finalized hash");
      if (request.finality === "submit-and-finalize" && result.lifecycle !== undefined) {
        validateLifecycle(result.lifecycle);
        if (result.lifecycle.state !== "finalized" || result.lifecycle.block_hash !== result.finalizedHash)
          throw new ProductSdkError("runtime_rejected", "submit route lifecycle is not bound to finalization");
        if (result.extrinsicHash !== undefined && result.lifecycle.extrinsic_hash !== result.extrinsicHash)
          throw new ProductSdkError("runtime_rejected", "submit route extrinsic evidence mismatch");
      }
      outcome = "success"; return result;
    } catch (error: any) { outcome = error instanceof ProductSdkError ? error.code : "runtime_rejected"; throw error; }
    finally { if(ownsId){
      this.inflight.delete(request.request_id);
      if (this.consents.get(request.consent.nonce)?.claimedBy === request.request_id) {
        this.consents.delete(request.consent.nonce); this.consumedNonces.add(request.consent.nonce);
      }
      this.finish(request.request_id,outcome)
    } }
  }
}
export function validateHostRequest(request: HostRequest): void {
  const exact=(o:any,keys:string[])=>o&&typeof o==="object"&&!Array.isArray(o)&&Object.keys(o).sort().join()===keys.sort().join();
  if(!exact(request,["version","request_id","application_id","capability","method","network","finality","payload","consent"])||request.version!==1||typeof request.request_id!=="string"||request.request_id.length<16||request.request_id.length>128||typeof request.application_id!=="string"||!request.application_id||typeof request.capability!=="string"||typeof request.method!=="string"||!["finalized","submit-and-finalize"].includes(request.finality))throw new ProductSdkError("invalid_input","invalid host request envelope");
  if(!exact(request.network,["genesis_hash","spec_version","transaction_version","metadata_hash","descriptor_contract_sha256","chain_spec_source_sha256","activation_state","production_activation_ready","access_mode"])||!Number.isInteger(request.network.spec_version)||!Number.isInteger(request.network.transaction_version)||!(["candidate-pending","production-approved"] as unknown[]).includes(request.network.activation_state)||typeof request.network.production_activation_ready!=="boolean"||!(["candidate","production"] as unknown[]).includes(request.network.access_mode))throw new ProductSdkError("invalid_input","invalid network envelope");
  if(!exact(request.consent,["scope","expires_at","nonce"])||!Array.isArray(request.consent.scope)||!request.consent.scope.length||request.consent.scope.some(x=>typeof x!=="string"||!x)||new Set(request.consent.scope).size!==request.consent.scope.length||!Number.isInteger(request.consent.expires_at)||request.consent.expires_at<1||typeof request.consent.nonce!=="string"||request.consent.nonce.length<16||request.consent.nonce.length>128)throw new ProductSdkError("invalid_input","invalid consent envelope");
  if(!request.payload||typeof request.payload!=="object"||Array.isArray(request.payload))throw new ProductSdkError("invalid_input","payload must be an object");
  assertRuntimeIdentity(request.network.genesis_hash, request.network.spec_version, request.network.transaction_version,request.network.metadata_hash,request.network.descriptor_contract_sha256,request.network.chain_spec_source_sha256,request.network.activation_state,request.network.production_activation_ready,request.network.access_mode);
  assertMethodPayload(request.capability, request.method, request.payload);
  assertMethodFinality(request.capability, request.method, request.finality);
}
export function assertCompositeSnapshot(hashes: string[]): string {
  if (!hashes.length || new Set(hashes).size !== 1) throw new ProductSdkError("inconsistent_snapshot", "composite read crossed finalized hashes");
  return hashes[0];
}
