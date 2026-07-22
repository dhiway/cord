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


import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import {
  ASSETS_NATIVE_BINDINGS,
  ASSETS_RUNTIME_GAPS,
} from "@cord-network/origin-sdk-assets";

const descriptor = readFileSync(
  new URL("../../packages/descriptors/chains/commons/generated/dist/commons.d.ts", import.meta.url),
  "utf8",
);
const runtime = readFileSync(
  new URL("../../../origin/orbis/runtime/src/lib.rs", import.meta.url),
  "utf8",
);

function descriptorHas(target: string, kind: "read" | "write"): boolean {
  const [pallet, method] = target.split(".");
  if (!pallet || !method) return false;
  const descriptorKind = kind === "read" ? "StorageDescriptor" : "TxDescriptor";
  return descriptor.includes(`${pallet}: {`) && descriptor.includes(`${method}: ${descriptorKind}<`);
}

test("every admitted assets pallet target exists in generated Commons metadata", () => {
  for (const target of ASSETS_NATIVE_BINDINGS.reads.filter((entry) => !entry.includes("Api."))) {
    assert.equal(descriptorHas(target, "read"), true, `missing storage target ${target}`);
  }
  for (const target of ASSETS_NATIVE_BINDINGS.writes) {
    assert.equal(descriptorHas(target, "write"), true, `missing call target ${target}`);
  }
});

test("payment estimates and asset-fee selection bind to current Commons runtime code", () => {
  assert.match(runtime, /impl pallet_transaction_payment_rpc_runtime_api::TransactionPaymentCallApi/);
  assert.match(runtime, /fn query_call_info\(/);
  assert.match(runtime, /fn query_call_fee_details\(/);
  assert.match(runtime, /ChargeAssetTxPayment<Runtime>/);
  assert.equal(ASSETS_NATIVE_BINDINGS.signedExtensions[0], "ChargeAssetTxPayment");
});

test("conversion quotes remain fail-closed until Commons implements their runtime API", () => {
  assert.doesNotMatch(runtime, /impl pallet_asset_conversion::AssetConversionApi/);
  assert.equal(ASSETS_RUNTIME_GAPS[0].target, "AssetConversionApi.*");
});
