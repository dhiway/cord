import { finalizedRead, submitAndFinalize, type RequestContext } from "./host.ts";
import type { JsonObject } from "./errors.ts";
import type { AccountId, BlockNumber, Hash32 } from "./types.ts";

export type SponsorableCapability = "identity" | "attestation" | "dotns" | "storage";

/** A closed, typed native route. No call bytes, SCALE, indices, or ABI are exposed. */
export interface SponsoredNativeTarget {
  readonly capability: SponsorableCapability;
  readonly method: string;
  readonly payload: JsonObject;
}

export interface SponsoredMortality {
  readonly valid_from: BlockNumber;
  readonly valid_until: BlockNumber;
}

export interface PrepareSponsoredIntentInput {
  readonly participant: AccountId;
  readonly nonce: string;
  readonly mortality: SponsoredMortality;
  readonly target: SponsoredNativeTarget;
}

/** Host/offline signing envelope returned by the typed v8 transport. */
export interface SponsoredIntentEnvelope extends PrepareSponsoredIntentInput {
  readonly version: 1;
  /** The domain committed by Orbis `IntentPreimageV7`. */
  readonly signing_domain: "orbis/meta-intent/v7";
  readonly genesis_hash: Hash32;
  readonly spec_version: number;
  readonly transaction_version: number;
  readonly metadata_hash: Hash32;
  /** Blake2-256 payload produced by the active typed metadata-v8 adapter for wallet signing. */
  readonly signing_payload_hash: Hash32;
  readonly intent_id: Hash32;
}

export type ParticipantSignatureScheme = "sr25519" | "ed25519" | "ecdsa";
export interface ParticipantSignature {
  readonly scheme: ParticipantSignatureScheme;
  /** Signature over the exact active-wire `envelope.signing_payload_hash`. */
  readonly value: string;
}

/** Participant signs the inner intent; the host transport's chain signer is the outer sponsor. */
export interface SignedSponsoredIntent {
  readonly envelope: SponsoredIntentEnvelope;
  readonly participant_signature: ParticipantSignature;
}

export interface SponsoredDispatchOutcome {
  readonly version: 1;
  readonly intent_id: Hash32;
  readonly participant: AccountId;
  readonly sponsor: AccountId;
  readonly dispatched: true;
  readonly meta_tx_event: "Dispatched";
  readonly inner_result: "Ok";
}

export const sponsoredTransaction = {
  prepareSponsoredIntent(context: RequestContext, input: PrepareSponsoredIntentInput) {
    return finalizedRead("transaction", context, "transaction", "prepare_sponsored_intent", {
      participant: input.participant,
      nonce: input.nonce,
      mortality: { ...input.mortality },
      target: {
        capability: input.target.capability,
        method: input.target.method,
        payload: { ...input.target.payload },
      },
    });
  },

  submitSponsoredIntent(context: RequestContext, signed_intent: SignedSponsoredIntent) {
    return submitAndFinalize("transaction", context, "transaction", "submit_sponsored_intent", {
      signed_intent: {
        envelope: {
          ...signed_intent.envelope,
          mortality: { ...signed_intent.envelope.mortality },
          target: {
            capability: signed_intent.envelope.target.capability,
            method: signed_intent.envelope.target.method,
            payload: { ...signed_intent.envelope.target.payload },
          },
        },
        participant_signature: { ...signed_intent.participant_signature },
      },
    });
  },
} as const;
