import { invalidDomainInput } from "./errors.ts";
import { finalizedRead, submitAndFinalize, type RequestContext } from "./host.ts";
import {
  page,
  type AccountId,
  type AgreementId,
  type BlockNumber,
  type ChallengeId,
  type ContainerId,
  type ContentCommitment,
  type DecimalU64,
  type IdPage,
  type PageInput,
  type ProviderId,
  type ReservationId,
} from "./types.ts";

declare const providerType: unique symbol;
export type ProviderEndpoint = string & { readonly [providerType]: "ProviderEndpoint" };
export type ProviderServiceKey = string & { readonly [providerType]: "ProviderServiceKey" };

export type ProviderStatus = "active" | "suspended";
export type AgreementStatus = "proposed" | "active" | "cancelled" | "expired";
export type ChallengeStatus = "open" | "proved" | "timed_out";

const utf8 = new TextEncoder();
function boundedProviderText<Kind extends "ProviderEndpoint" | "ProviderServiceKey">(
  value: string,
  kind: Kind,
  maxBytes: number,
): string & { readonly [providerType]: Kind } {
  if (!value || utf8.encode(value).length > maxBytes) {
    invalidDomainInput("provider", kind, `${kind} must contain 1-${maxBytes} UTF-8 bytes`);
  }
  return value as string & { readonly [providerType]: Kind };
}

export const providerEndpoint = (value: string): ProviderEndpoint =>
  boundedProviderText(value, "ProviderEndpoint", 512);
export const providerServiceKey = (value: string): ProviderServiceKey =>
  boundedProviderText(value, "ProviderServiceKey", 128);

export interface ProviderView {
  readonly provider: ProviderId;
  readonly endpoint: ProviderEndpoint;
  readonly service_key: ProviderServiceKey;
  readonly capacity_bytes: DecimalU64;
  readonly allocated_bytes: DecimalU64;
  readonly pending_bytes: DecimalU64;
  readonly status: ProviderStatus;
  readonly last_heartbeat: BlockNumber;
  readonly reputation: number;
}

export interface AgreementView {
  readonly agreement_id: AgreementId;
  readonly owner: AccountId;
  readonly provider: ProviderId;
  readonly container_ref: ContainerId;
  readonly content_commitment: ContentCommitment;
  readonly reservation_ref: ReservationId | null;
  readonly bytes: DecimalU64;
  readonly created_at: BlockNumber;
  readonly expires_at: BlockNumber;
  readonly pending_expiry: BlockNumber | null;
  readonly status: AgreementStatus;
}

export interface ChallengeView {
  readonly challenge_id: ChallengeId;
  readonly provider: ProviderId;
  readonly agreement_id: AgreementId;
  readonly expected_commitment: ContentCommitment;
  readonly due_at: BlockNumber;
  readonly proof_commitment: ContentCommitment | null;
  readonly status: ChallengeStatus;
}

export interface ProviderCheckpoint {
  readonly challenge_id: ChallengeId;
  readonly proof_commitment: ContentCommitment;
  readonly recorded_at: BlockNumber;
}

export interface ProviderRootView {
  readonly sequence: DecimalU64;
  readonly root: ContentCommitment;
  readonly leaf_count: DecimalU64;
  readonly committed_at: BlockNumber;
}

export interface DeletionAcknowledgementView {
  readonly provider: ProviderId;
  readonly content_commitment: ContentCommitment;
  readonly tombstone_root: ContentCommitment;
  readonly root_sequence: DecimalU64;
  readonly leaf_index: DecimalU64;
  readonly leaf_count: DecimalU64;
  readonly proof_commitment: ContentCommitment;
  readonly acknowledged_at: BlockNumber;
}

export const provider = {
  providerById(context: RequestContext, provider: ProviderId) {
    return finalizedRead("provider", context, "storage", "provider_by_id", { provider });
  },

  providers(context: RequestContext, input?: PageInput) {
    return finalizedRead("provider", context, "storage", "providers", { ...page(input) });
  },

  agreementById(context: RequestContext, agreement_id: AgreementId) {
    return finalizedRead("provider", context, "storage", "agreement_by_id", { agreement_id });
  },

  providerAgreements(context: RequestContext, provider: ProviderId, input?: PageInput) {
    return finalizedRead("provider", context, "storage", "provider_agreements", {
      provider,
      ...page(input),
    });
  },

  ownerAgreements(context: RequestContext, owner: AccountId, input?: PageInput) {
    return finalizedRead("provider", context, "storage", "owner_agreements", {
      owner,
      ...page(input),
    });
  },

  containerAgreements(context: RequestContext, container_ref: ContainerId, input?: PageInput) {
    return finalizedRead("provider", context, "storage", "container_agreements", {
      container_ref,
      ...page(input),
    });
  },

  agreementNonce(context: RequestContext, owner: AccountId) {
    return finalizedRead("provider", context, "storage", "agreement_nonce", { owner });
  },

  challengeById(context: RequestContext, challenge_id: ChallengeId) {
    return finalizedRead("provider", context, "storage", "challenge_by_id", { challenge_id });
  },

  challengesAt(context: RequestContext, block: BlockNumber, input?: PageInput) {
    return finalizedRead("provider", context, "storage", "challenges_at", {
      block,
      ...page(input),
    });
  },

  openChallengeCount(context: RequestContext, agreement_id: AgreementId) {
    return finalizedRead("provider", context, "storage", "open_challenge_count", { agreement_id });
  },

  canAcceptCapacity(context: RequestContext, provider: ProviderId, additional_bytes: DecimalU64) {
    return finalizedRead("provider", context, "storage", "can_accept_capacity", {
      provider,
      additional_bytes,
    });
  },

  providerCheckpoint(context: RequestContext, provider: ProviderId) {
    return finalizedRead("provider", context, "storage", "provider_checkpoint", { provider });
  },

  providerRoot(context: RequestContext, provider: ProviderId) {
    return finalizedRead("provider", context, "storage", "provider_root", { provider });
  },

  deletionAcknowledgement(context: RequestContext, agreement_id: AgreementId) {
    return finalizedRead("provider", context, "storage", "deletion_acknowledgement", {
      agreement_id,
    });
  },

  registerProvider(
    context: RequestContext,
    provider: ProviderId,
    endpoint: ProviderEndpoint,
    service_key: ProviderServiceKey,
    capacity_bytes: DecimalU64,
  ) {
    return submitAndFinalize("provider", context, "storage", "register_provider", {
      provider,
      endpoint,
      service_key,
      capacity_bytes,
    });
  },

  updateProvider(
    context: RequestContext,
    provider: ProviderId,
    endpoint: ProviderEndpoint,
    service_key: ProviderServiceKey,
    capacity_bytes: DecimalU64,
  ) {
    return submitAndFinalize("provider", context, "storage", "update_provider", {
      provider,
      endpoint,
      service_key,
      capacity_bytes,
    });
  },

  setProviderStatus(context: RequestContext, provider: ProviderId, status: ProviderStatus) {
    return submitAndFinalize("provider", context, "storage", "set_provider_status", { provider, status });
  },

  removeProvider(context: RequestContext, provider: ProviderId) {
    return submitAndFinalize("provider", context, "storage", "remove_provider", { provider });
  },

  heartbeat(context: RequestContext) {
    return submitAndFinalize("provider", context, "storage", "heartbeat", {});
  },

  proposeAgreement(
    context: RequestContext,
    input: {
      readonly provider: ProviderId;
      readonly container_ref: ContainerId;
      readonly content_commitment: ContentCommitment;
      readonly reservation_ref: ReservationId | null;
      readonly bytes: DecimalU64;
      readonly expires_at: BlockNumber;
    },
  ) {
    return submitAndFinalize("provider", context, "storage", "propose_agreement", { ...input });
  },

  acceptAgreement(context: RequestContext, agreement_id: AgreementId) {
    return submitAndFinalize("provider", context, "storage", "accept_agreement", { agreement_id });
  },

  cancelAgreement(context: RequestContext, agreement_id: AgreementId) {
    return submitAndFinalize("provider", context, "storage", "cancel_agreement", { agreement_id });
  },

  issueChallenge(
    context: RequestContext,
    agreement_id: AgreementId,
    expected_commitment: ContentCommitment,
    due_at: BlockNumber,
  ) {
    return submitAndFinalize("provider", context, "storage", "issue_challenge", {
      agreement_id,
      expected_commitment,
      due_at,
    });
  },

  submitCheckpoint(
    context: RequestContext,
    challenge_id: ChallengeId,
    proof_commitment: ContentCommitment,
  ) {
    return submitAndFinalize("provider", context, "storage", "submit_checkpoint", {
      challenge_id,
      proof_commitment,
    });
  },

  timeoutChallenge(context: RequestContext, challenge_id: ChallengeId) {
    return submitAndFinalize("provider", context, "storage", "timeout_challenge", { challenge_id });
  },

  requestRenewal(context: RequestContext, agreement_id: AgreementId, expires_at: BlockNumber) {
    return submitAndFinalize("provider", context, "storage", "request_renewal", {
      agreement_id,
      expires_at,
    });
  },

  acceptRenewal(context: RequestContext, agreement_id: AgreementId) {
    return submitAndFinalize("provider", context, "storage", "accept_renewal", { agreement_id });
  },

  expireAgreement(context: RequestContext, agreement_id: AgreementId) {
    return submitAndFinalize("provider", context, "storage", "expire_agreement", { agreement_id });
  },

  pruneAgreement(context: RequestContext, agreement_id: AgreementId) {
    return submitAndFinalize("provider", context, "storage", "prune_agreement", { agreement_id });
  },

  acknowledgeDeletion(
    context: RequestContext,
    agreement_id: AgreementId,
    content_commitment: ContentCommitment,
    tombstone_root: ContentCommitment,
    root_sequence: DecimalU64,
    leaf_index: DecimalU64,
    leaf_count: DecimalU64,
    inclusion_proof: readonly ContentCommitment[],
  ) {
    return submitAndFinalize("provider", context, "storage", "acknowledge_deletion", {
      agreement_id,
      content_commitment,
      tombstone_root,
      root_sequence,
      leaf_index,
      leaf_count,
      inclusion_proof: [...inclusion_proof],
    });
  },

  commitProviderRoot(
    context: RequestContext,
    sequence: DecimalU64,
    appended_leaves: readonly ContentCommitment[],
  ) {
    return submitAndFinalize("provider", context, "storage", "commit_provider_root", {
      sequence,
      appended_leaves: [...appended_leaves],
    });
  },
} as const;

export type ProviderIdPage = IdPage<ProviderId>;
export type AgreementIdPage = IdPage<AgreementId>;
