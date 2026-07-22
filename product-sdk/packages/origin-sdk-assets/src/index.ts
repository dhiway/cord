// This file is part of CORD – https://cord.network

// Copyright (C) Dhiway Networks Pvt. Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later

// CORD is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// CORD is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with CORD. If not, see <https://www.gnu.org/licenses/>.


import type { CommonsChainClient } from "@cord-network/origin-sdk-chain-client";
import { COMMONS_NETWORK_BINDING } from "@cord-network/origin-sdk-descriptors";
import type { SdkResult } from "@cord-network/origin-sdk-errors";
import { accountId, type AccountId } from "@cord-network/origin-sdk-identity";
import { ok } from "@cord-network/origin-sdk-result";
import { prepareAtFinalized, type PreparedTransaction } from "@cord-network/origin-sdk-tx";

declare const assetType: unique symbol;
export type AssetId = number & { readonly [assetType]: "AssetId" };
export type CollectionId = number & { readonly [assetType]: "CollectionId" };
export type ItemId = number & { readonly [assetType]: "ItemId" };
export type Balance = bigint & { readonly [assetType]: "Balance" };
export type AssetMetadataBytes = Uint8Array & { readonly [assetType]: "AssetMetadataBytes" };

/** Portable XCM v5 junctions supported by the Commons application asset surface. */
export type AssetJunction =
  | { readonly type: "Parachain"; readonly value: number }
  | { readonly type: "PalletInstance"; readonly value: number }
  | { readonly type: "GeneralIndex"; readonly value: bigint };

/** Adapter-neutral representation of an XCM v5 asset location. */
export interface AssetLocation {
  readonly parents: number;
  readonly junctions: readonly AssetJunction[];
}

export interface AssetPaymentOptions {
  readonly feeAsset: AssetLocation | null;
  readonly tip: Balance;
}

export interface NftMintWitness {
  readonly ownedItem?: ItemId | null;
  readonly mintPrice?: Balance | null;
}

export interface PaymentInfo {
  readonly partial_fee: Balance;
  readonly weight_ref_time: bigint;
  readonly weight_proof_size: bigint;
  readonly class: "normal" | "operational" | "mandatory";
}

export type AssetReadRequest = {
  readonly kind: "read";
  readonly target: string;
  readonly payload: Readonly<Record<string, unknown>>;
};
export type AssetWriteRequest = {
  readonly kind: "write";
  readonly target: string;
  readonly payload: Readonly<Record<string, unknown>>;
};

const read = (target: string, payload: Readonly<Record<string, unknown>>): AssetReadRequest =>
  ({ kind: "read", target, payload });
const write = (target: string, payload: Readonly<Record<string, unknown>>): AssetWriteRequest =>
  ({ kind: "write", target, payload });

function u32(value: number, label: string): number {
  if (!Number.isSafeInteger(value) || value < 0 || value > 0xffff_ffff) {
    throw new TypeError(`${label} must be a u32`);
  }
  return value;
}

function u8(value: number, label: string): number {
  if (!Number.isSafeInteger(value) || value < 0 || value > 0xff) {
    throw new TypeError(`${label} must be a u8`);
  }
  return value;
}

function amount(value: bigint | number | string, label: string): Balance {
  const result = BigInt(value);
  if (result < 0n || result > 0xffff_ffff_ffff_ffff_ffff_ffff_ffff_ffffn) {
    throw new TypeError(`${label} must be a u128`);
  }
  return result as Balance;
}

export const assetId = (value: number): AssetId => u32(value, "asset id") as AssetId;
export const collectionId = (value: number): CollectionId =>
  u32(value, "collection id") as CollectionId;
export const itemId = (value: number): ItemId => u32(value, "item id") as ItemId;
export const balance = (value: bigint | number | string): Balance => amount(value, "balance");

export function metadataBytes(value: Uint8Array): AssetMetadataBytes {
  if (!(value instanceof Uint8Array) || value.length > 50) {
    throw new TypeError("asset metadata must contain at most 50 bytes");
  }
  return value.slice() as AssetMetadataBytes;
}

function cloneLocation(location: AssetLocation): AssetLocation {
  return {
    parents: location.parents,
    junctions: location.junctions.map((junction) => ({ ...junction })),
  };
}

export function assetLocation(
  parents: number,
  junctions: readonly AssetJunction[],
): AssetLocation {
  u8(parents, "asset location parents");
  if (junctions.length > 8) throw new TypeError("asset location supports at most 8 junctions");
  for (const junction of junctions) {
    if (junction.type === "GeneralIndex") amount(junction.value, "general index");
    else u32(junction.value, junction.type === "Parachain" ? "parachain" : "pallet instance");
    if (junction.type === "PalletInstance") u8(junction.value, "pallet instance");
  }
  return cloneLocation({ parents, junctions });
}

/** Foundation relay asset as seen from Commons. */
export const foundationAsset = (): AssetLocation => assetLocation(1, []);

/** Commons trust-backed asset represented by `Assets` pallet index 80 and a compact ID. */
export const commonsAsset = (id: AssetId): AssetLocation => assetLocation(0, [
  { type: "PalletInstance", value: 80 },
  { type: "GeneralIndex", value: BigInt(assetId(id)) },
]);

function clonePath(path: readonly AssetLocation[]): readonly AssetLocation[] {
  return path.map(cloneLocation);
}

function validPath(path: readonly AssetLocation[]): void {
  if (path.length < 2 || path.length > 4) {
    throw new TypeError("asset conversion path must contain 2-4 locations");
  }
  for (const location of path) assetLocation(location.parents, location.junctions);
}

export function paymentOptions(
  feeAsset: AssetLocation | null = null,
  tip: bigint | number | string = 0n,
): AssetPaymentOptions {
  return {
    feeAsset: feeAsset === null ? null : cloneLocation(feeAsset),
    tip: amount(tip, "tip"),
  };
}

export const assetReads = {
  nativeAccount(account: AccountId) {
    accountId(account);
    return read("System.Account", { account });
  },
  details(id: AssetId) {
    return read("Assets.Asset", { id: assetId(id) });
  },
  account(id: AssetId, account: AccountId) {
    accountId(account);
    return read("Assets.Account", { id: assetId(id), account });
  },
  metadata(id: AssetId) {
    return read("Assets.Metadata", { id: assetId(id) });
  },
  collection(collection: CollectionId) {
    return read("Nfts.Collection", { collection: collectionId(collection) });
  },
  item(collection: CollectionId, item: ItemId) {
    return read("Nfts.Item", { collection: collectionId(collection), item: itemId(item) });
  },
  pool(asset1: AssetLocation, asset2: AssetLocation) {
    validPath([asset1, asset2]);
    return read("AssetConversion.Pools", {
      asset1: cloneLocation(asset1),
      asset2: cloneLocation(asset2),
    });
  },
  paymentInfo(call: Readonly<Record<string, unknown>>, encodedLength: number) {
    return read("TransactionPaymentCallApi.query_call_info", {
      call: { ...call },
      len: u32(encodedLength, "encoded transaction length"),
    });
  },
  paymentFeeDetails(call: Readonly<Record<string, unknown>>, encodedLength: number) {
    return read("TransactionPaymentCallApi.query_call_fee_details", {
      call: { ...call },
      len: u32(encodedLength, "encoded transaction length"),
    });
  },
} as const;

export const assetWrites = {
  transferNative(dest: AccountId, value: Balance, keepAlive = true) {
    accountId(dest);
    return write(keepAlive ? "Balances.transfer_keep_alive" : "Balances.transfer_allow_death", {
      dest,
      value: amount(value, "value"),
    });
  },
  transferAllNative(dest: AccountId, keepAlive = true) {
    accountId(dest);
    return write("Balances.transfer_all", { dest, keep_alive: keepAlive });
  },
  burnNative(value: Balance, keepAlive = true) {
    return write("Balances.burn", { value: amount(value, "value"), keep_alive: keepAlive });
  },
  create(id: AssetId, admin: AccountId, minBalance: Balance) {
    accountId(admin);
    return write("Assets.create", {
      id: assetId(id),
      admin,
      min_balance: amount(minBalance, "min balance"),
    });
  },
  startDestroy(id: AssetId) {
    return write("Assets.start_destroy", { id: assetId(id) });
  },
  mint(id: AssetId, beneficiary: AccountId, value: Balance) {
    accountId(beneficiary);
    return write("Assets.mint", {
      id: assetId(id), beneficiary, amount: amount(value, "amount"),
    });
  },
  burn(id: AssetId, who: AccountId, value: Balance) {
    accountId(who);
    return write("Assets.burn", { id: assetId(id), who, amount: amount(value, "amount") });
  },
  transfer(id: AssetId, target: AccountId, value: Balance, keepAlive = false) {
    accountId(target);
    return write(keepAlive ? "Assets.transfer_keep_alive" : "Assets.transfer", {
      id: assetId(id), target, amount: amount(value, "amount"),
    });
  },
  approve(id: AssetId, delegate: AccountId, value: Balance) {
    accountId(delegate);
    return write("Assets.approve_transfer", {
      id: assetId(id), delegate, amount: amount(value, "amount"),
    });
  },
  cancelApproval(id: AssetId, delegate: AccountId) {
    accountId(delegate);
    return write("Assets.cancel_approval", { id: assetId(id), delegate });
  },
  transferApproved(
    id: AssetId,
    owner: AccountId,
    destination: AccountId,
    value: Balance,
  ) {
    accountId(owner);
    accountId(destination);
    return write("Assets.transfer_approved", {
      id: assetId(id), owner, destination, amount: amount(value, "amount"),
    });
  },
  setMetadata(
    id: AssetId,
    name: Uint8Array,
    symbol: Uint8Array,
    decimals: number,
  ) {
    return write("Assets.set_metadata", {
      id: assetId(id),
      name: metadataBytes(name),
      symbol: metadataBytes(symbol),
      decimals: u8(decimals, "decimals"),
    });
  },
  clearMetadata(id: AssetId) {
    return write("Assets.clear_metadata", { id: assetId(id) });
  },
  transferOwnership(id: AssetId, owner: AccountId) {
    accountId(owner);
    return write("Assets.transfer_ownership", { id: assetId(id), owner });
  },
  createCollection(admin: AccountId, config: Readonly<Record<string, unknown>>) {
    accountId(admin);
    return write("Nfts.create", { admin, config: { ...config } });
  },
  mintNft(
    collection: CollectionId,
    item: ItemId,
    owner: AccountId,
    witnessData: NftMintWitness | null = null,
  ) {
    accountId(owner);
    const witness_data = witnessData === null ? null : {
      owned_item: witnessData.ownedItem ?? null,
      mint_price: witnessData.mintPrice === undefined || witnessData.mintPrice === null
        ? null
        : amount(witnessData.mintPrice, "mint price"),
    };
    return write("Nfts.mint", {
      collection: collectionId(collection),
      item: itemId(item),
      mint_to: owner,
      witness_data,
    });
  },
  transferNft(collection: CollectionId, item: ItemId, dest: AccountId) {
    accountId(dest);
    return write("Nfts.transfer", {
      collection: collectionId(collection), item: itemId(item), dest,
    });
  },
  burnNft(collection: CollectionId, item: ItemId) {
    return write("Nfts.burn", { collection: collectionId(collection), item: itemId(item) });
  },
  setNftPrice(
    collection: CollectionId,
    item: ItemId,
    price: Balance | null,
    whitelistedBuyer: AccountId | null = null,
  ) {
    if (whitelistedBuyer !== null) accountId(whitelistedBuyer);
    return write("Nfts.set_price", {
      collection: collectionId(collection),
      item: itemId(item),
      price: price === null ? null : amount(price, "price"),
      whitelisted_buyer: whitelistedBuyer,
    });
  },
  buyNft(collection: CollectionId, item: ItemId, bidPrice: Balance) {
    return write("Nfts.buy_item", {
      collection: collectionId(collection),
      item: itemId(item),
      bid_price: amount(bidPrice, "bid price"),
    });
  },
  createPool(asset1: AssetLocation, asset2: AssetLocation) {
    validPath([asset1, asset2]);
    return write("AssetConversion.create_pool", {
      asset1: cloneLocation(asset1), asset2: cloneLocation(asset2),
    });
  },
  addLiquidity(
    asset1: AssetLocation,
    asset2: AssetLocation,
    amount1: Balance,
    amount2: Balance,
    min1: Balance,
    min2: Balance,
    mintTo: AccountId,
  ) {
    validPath([asset1, asset2]);
    accountId(mintTo);
    return write("AssetConversion.add_liquidity", {
      asset1: cloneLocation(asset1),
      asset2: cloneLocation(asset2),
      amount1_desired: amount(amount1, "amount1"),
      amount2_desired: amount(amount2, "amount2"),
      amount1_min: amount(min1, "min1"),
      amount2_min: amount(min2, "min2"),
      mint_to: mintTo,
    });
  },
  removeLiquidity(
    asset1: AssetLocation,
    asset2: AssetLocation,
    lpTokenBurn: Balance,
    min1: Balance,
    min2: Balance,
    withdrawTo: AccountId,
  ) {
    validPath([asset1, asset2]);
    accountId(withdrawTo);
    return write("AssetConversion.remove_liquidity", {
      asset1: cloneLocation(asset1),
      asset2: cloneLocation(asset2),
      lp_token_burn: amount(lpTokenBurn, "LP token burn"),
      amount1_min_receive: amount(min1, "minimum asset1 receive"),
      amount2_min_receive: amount(min2, "minimum asset2 receive"),
      withdraw_to: withdrawTo,
    });
  },
  swapExactInput(
    path: readonly AssetLocation[],
    input: Balance,
    minOutput: Balance,
    sendTo: AccountId,
    keepAlive = true,
  ) {
    validPath(path);
    accountId(sendTo);
    return write("AssetConversion.swap_exact_tokens_for_tokens", {
      path: clonePath(path),
      amount_in: amount(input, "input"),
      amount_out_min: amount(minOutput, "minimum output"),
      send_to: sendTo,
      keep_alive: keepAlive,
    });
  },
  swapExactOutput(
    path: readonly AssetLocation[],
    output: Balance,
    maxInput: Balance,
    sendTo: AccountId,
    keepAlive = true,
  ) {
    validPath(path);
    accountId(sendTo);
    return write("AssetConversion.swap_tokens_for_exact_tokens", {
      path: clonePath(path),
      amount_out: amount(output, "output"),
      amount_in_max: amount(maxInput, "maximum input"),
      send_to: sendTo,
      keep_alive: keepAlive,
    });
  },
} as const;

export interface AssetsRuntimeAdapter {
  read<T>(
    at: `0x${string}`,
    target: string,
    payload: Readonly<Record<string, unknown>>,
    signal?: AbortSignal,
  ): Promise<T>;
  prepare(
    at: `0x${string}`,
    target: string,
    payload: Readonly<Record<string, unknown>>,
    payment: AssetPaymentOptions,
    signal?: AbortSignal,
  ): Promise<PreparedTransaction>;
}

export interface AssetsClient {
  read<T = unknown>(
    request: AssetReadRequest,
    signal?: AbortSignal,
  ): Promise<SdkResult<T>>;
  readTogether(
    requests: readonly AssetReadRequest[],
    signal?: AbortSignal,
  ): Promise<SdkResult<{
    readonly finalized_hash: `0x${string}`;
    readonly finalized_number: bigint;
    readonly values: readonly unknown[];
  }>>;
  prepare(
    request: AssetWriteRequest,
    payment?: AssetPaymentOptions,
    signal?: AbortSignal,
  ): Promise<SdkResult<PreparedTransaction>>;
}

export function createAssetsClient(
  chain: CommonsChainClient,
  runtime: AssetsRuntimeAdapter,
): AssetsClient {
  return {
    read: (request, signal) => chain.readFinalized(
      (at) => runtime.read(at, request.target, request.payload, signal), signal,
    ),
    async readTogether(requests, signal) {
      const snapshot = await chain.finalizedSnapshot(signal);
      if (!snapshot.success) return snapshot;
      const values = [];
      for (const request of requests) {
        const result = await snapshot.value.read(
          (at) => runtime.read(at, request.target, request.payload, signal), signal,
        );
        if (!result.success) return result;
        values.push(result.value);
      }
      return ok({
        finalized_hash: snapshot.value.block.hash,
        finalized_number: snapshot.value.block.number,
        values,
      });
    },
    prepare: (request, payment = paymentOptions(), signal) => prepareAtFinalized(
      chain,
      ({ block }) => runtime.prepare(
        block.hash, request.target, request.payload, payment, signal,
      ),
      signal,
    ),
  };
}

export const ASSETS_NATIVE_BINDINGS = {
  metadataHash: COMMONS_NETWORK_BINDING.metadata_hash,
  pallets: ["System", "Balances", "Assets", "Nfts", "AssetConversion", "AssetTxPayment"],
  runtimeApis: ["TransactionPaymentCallApi"],
  signedExtensions: ["ChargeAssetTxPayment"],
  reads: [
    "System.Account",
    "Assets.Asset",
    "Assets.Account",
    "Assets.Metadata",
    "Nfts.Collection",
    "Nfts.Item",
    "AssetConversion.Pools",
    "TransactionPaymentCallApi.query_call_info",
    "TransactionPaymentCallApi.query_call_fee_details",
  ],
  writes: [
    "Balances.transfer_keep_alive",
    "Balances.transfer_allow_death",
    "Balances.transfer_all",
    "Balances.burn",
    "Assets.create",
    "Assets.start_destroy",
    "Assets.mint",
    "Assets.burn",
    "Assets.transfer_keep_alive",
    "Assets.transfer",
    "Assets.approve_transfer",
    "Assets.cancel_approval",
    "Assets.transfer_approved",
    "Assets.set_metadata",
    "Assets.clear_metadata",
    "Assets.transfer_ownership",
    "Nfts.create",
    "Nfts.mint",
    "Nfts.transfer",
    "Nfts.burn",
    "Nfts.set_price",
    "Nfts.buy_item",
    "AssetConversion.create_pool",
    "AssetConversion.add_liquidity",
    "AssetConversion.remove_liquidity",
    "AssetConversion.swap_exact_tokens_for_tokens",
    "AssetConversion.swap_tokens_for_exact_tokens",
  ],
  canonicalNft: "Nfts",
  maxConversionPathLength: 4,
  assetMetadataStringLimit: 50,
} as const;

export const ASSETS_ADMIN_EXCLUSIONS = [
  { target: "Balances.force_transfer", reason: "root balance administration" },
  { target: "Balances.force_unreserve", reason: "root balance administration" },
  { target: "Balances.force_set_balance", reason: "root balance administration" },
  { target: "Assets.force_create", reason: "root asset administration" },
  { target: "Assets.force_asset_status", reason: "root asset administration" },
  { target: "Assets.force_set_metadata", reason: "root asset administration" },
  { target: "Nfts.force_create", reason: "root NFT administration" },
  { target: "Nfts.force_collection_owner", reason: "root NFT administration" },
  { target: "Nfts.force_collection_config", reason: "root NFT administration" },
] as const;

export const ASSETS_RUNTIME_GAPS = [
  {
    target: "AssetConversionApi.*",
    reason: "Commons does not currently implement the asset-conversion quote runtime API",
  },
] as const;

export const ASSETS_DUPLICATE_EXCLUSIONS = [
  {
    target: "Uniques.*",
    reason: "Nfts is the canonical Commons NFT application surface; remove duplicate runtime composition in P8",
  },
] as const;
