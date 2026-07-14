import { finalizedRead, submitAndFinalize, type RequestContext } from "./host.ts";
import type { JsonObject } from "./errors.ts";
import type { AccountId, BlockNumber, Hash32 } from "./types.ts";
import type { attestation } from "./attestation.ts";
import type { names } from "./names.ts";
import type { drive } from "./drive.ts";
import type { identity } from "./identity.ts";
import type { provider } from "./provider.ts";
import type { s3 } from "./s3.ts";
import type { storage } from "./storage.ts";

type NativeRouteFactory =
  | typeof attestation[keyof typeof attestation]
  | typeof names[keyof typeof names]
  | typeof drive[keyof typeof drive]
  | typeof identity[keyof typeof identity]
  | typeof provider[keyof typeof provider]
  | typeof s3[keyof typeof s3]
  | typeof storage[keyof typeof storage];

type NativeRouteRequestOf<Factory> = Factory extends (...args: never[]) => infer Request
  ? Request
  : never;
type NativeRouteRequest = NativeRouteRequestOf<NativeRouteFactory>;

type SponsoredTargetFromRequest<Request> = Request extends {
  readonly finality: "submit-and-finalize";
  readonly capability: infer Capability extends string;
  readonly method: infer Method extends string;
  readonly payload: infer Payload extends JsonObject;
}
  ? {
      readonly capability: Capability;
      readonly method: Method;
      readonly payload: Payload;
    }
  : never;

/**
 * Every sponsorable target is derived from an exported native write factory.
 * This keeps capability, method, and payload correlated at compile time and
 * leaves no stringly-typed SCALE, pallet/call index, or ABI escape hatch.
 */
export type SponsoredNativeTarget = SponsoredTargetFromRequest<NativeRouteRequest>;
export type SponsorableCapability = SponsoredNativeTarget["capability"];

declare const sponsoredNonceType: unique symbol;
/** Canonical decimal representation of the runtime's `u32` participant nonce. */
export type SponsoredNonce = string & { readonly [sponsoredNonceType]: "SponsoredNonceU32" };

export function sponsoredNonce(value: string | number): SponsoredNonce {
  const text = String(value);
  if (!/^(0|[1-9][0-9]{0,9})$/.test(text) || BigInt(text) > 0xffff_ffffn) {
    throw new TypeError("sponsored nonce must be a canonical unsigned 32-bit decimal string");
  }
  return text as SponsoredNonce;
}

export interface SponsoredMortality {
  readonly valid_from: BlockNumber;
  readonly valid_until: BlockNumber;
}

export interface PrepareSponsoredIntentInput {
  readonly participant: AccountId;
  readonly nonce: SponsoredNonce;
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
export type ParticipantSignature =
  | {
      readonly scheme: "sr25519";
      /** Lowercase 0x-prefixed 64-byte signature over `envelope.signing_payload_hash`. */
      readonly value: string;
    }
  | {
      readonly scheme: "ed25519";
      /** Lowercase 0x-prefixed 64-byte signature over `envelope.signing_payload_hash`. */
      readonly value: string;
    }
  | {
      readonly scheme: "ecdsa";
      /** Lowercase 0x-prefixed 65-byte signature over `envelope.signing_payload_hash`. */
      readonly value: string;
    };

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

function cloneTarget<Target extends SponsoredNativeTarget>(target: Target): Target {
  return { ...target, payload: { ...target.payload } };
}

export const sponsoredTransaction = {
  prepareSponsoredIntent(context: RequestContext, input: PrepareSponsoredIntentInput) {
    return finalizedRead("transaction", context, "transaction", "prepare_sponsored_intent", {
      participant: input.participant,
      nonce: input.nonce,
      mortality: { ...input.mortality },
      target: cloneTarget(input.target),
    });
  },

  submitSponsoredIntent(context: RequestContext, signed_intent: SignedSponsoredIntent) {
    return submitAndFinalize("transaction", context, "transaction", "submit_sponsored_intent", {
      signed_intent: {
        envelope: {
          ...signed_intent.envelope,
          mortality: { ...signed_intent.envelope.mortality },
          target: cloneTarget(signed_intent.envelope.target),
        },
        participant_signature: { ...signed_intent.participant_signature },
      },
    });
  },
} as const;
