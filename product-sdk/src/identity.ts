import { finalizedRead, submitAndFinalize, type RequestContext } from "./host.ts";
import type { AccountId, Hash32, Versioned } from "./types.ts";

/** Closed JSON representation of `pallet-orbis-people::Data`; never SCALE bytes. */
export type IdentityData =
  | { readonly kind: "none" }
  | { readonly kind: "raw"; readonly value: string }
  | { readonly kind: "blake2_256"; readonly hash: Hash32 }
  | { readonly kind: "sha2_256"; readonly hash: Hash32 }
  | { readonly kind: "keccak_256"; readonly hash: Hash32 }
  | { readonly kind: "sha3_256"; readonly hash: Hash32 };

export interface IdentityAdditionalField {
  readonly key: IdentityData;
  readonly value: IdentityData;
}

/** Product projection of the bounded native identity info. */
export interface IdentityInfo {
  readonly display: IdentityData;
  readonly legal: IdentityData;
  readonly web: IdentityData;
  readonly email: IdentityData;
  readonly image: IdentityData;
  readonly additional: readonly IdentityAdditionalField[];
}

export type IdentityJudgement =
  | "reasonable"
  | "known_good"
  | "out_of_date"
  | "low_quality"
  | "erroneous";

/** Privacy-preserving fixed-width projection returned by IdentityPersonhoodApi v1. */
export interface IdentityStatusView {
  readonly registered: boolean;
  readonly judgement_count: number;
  readonly requested: number;
  readonly reasonable: number;
  readonly known_good: number;
  readonly out_of_date: number;
  readonly low_quality: number;
  readonly erroneous: number;
}

export interface PersonhoodStatusView {
  readonly full_personal_id: string | null;
  readonly full_recognized: boolean;
  readonly lite_recognized: boolean;
}

export interface AttestationAllowanceView {
  readonly remaining: number;
}

export interface CandidateSignature {
  readonly scheme: "sr25519" | "ed25519" | "ecdsa";
  readonly bytes: string;
}
export type RingVrfKey = Hash32 & { readonly __ringVrfKey: unique symbol };
export type RingVrfProof = string & { readonly __ringVrfProof: unique symbol };

/** Closed lite-person call; consumer-registration forwarding is deliberately absent. */
export interface LitePersonAttestation {
  readonly candidate: AccountId;
  readonly candidate_signature: CandidateSignature;
  readonly ring_vrf_key: RingVrfKey;
  readonly proof_of_ownership: RingVrfProof;
}

export const identity = {
  identityStatus(context: RequestContext, account: AccountId) {
    return finalizedRead("identity", context, "identity", "identity_status", { account });
  },

  personhoodStatus(context: RequestContext, account: AccountId) {
    return finalizedRead("identity", context, "identity", "personhood_status", { account });
  },

  attestationAllowance(context: RequestContext, account: AccountId) {
    return finalizedRead("identity", context, "identity", "attestation_allowance", { account });
  },

  setIdentity(context: RequestContext, info: IdentityInfo) {
    return submitAndFinalize("identity", context, "identity", "set_identity", {
      info: { ...info, additional: info.additional.map(({ key, value }) => ({ key, value })) },
    });
  },

  clearIdentity(context: RequestContext) {
    return submitAndFinalize("identity", context, "identity", "clear_identity", {});
  },

  requestJudgement(context: RequestContext, registrar: AccountId) {
    return submitAndFinalize("identity", context, "identity", "request_judgement", { registrar });
  },

  cancelJudgementRequest(context: RequestContext, registrar: AccountId) {
    return submitAndFinalize("identity", context, "identity", "cancel_judgement_request", { registrar });
  },

  provideJudgement(
    context: RequestContext,
    target: AccountId,
    judgement: IdentityJudgement,
    identity_hash: Hash32,
  ) {
    return submitAndFinalize("identity", context, "identity", "provide_judgement", {
      target,
      judgement,
      identity_hash,
    });
  },

  attestLitePerson(context: RequestContext, input: LitePersonAttestation) {
    return submitAndFinalize("identity", context, "identity", "attest_lite_person", { ...input });
  },
} as const;

export type IdentityStatusResponse = Versioned<IdentityStatusView>;
export type PersonhoodStatusResponse = Versioned<PersonhoodStatusView>;
export type AttestationAllowanceResponse = Versioned<AttestationAllowanceView>;
