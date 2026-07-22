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

import assert from "node:assert/strict";import test from "node:test";import { submitAndFinalize,composeBatch } from "../src/index.ts";import { fakeTransaction } from "../src/testing.ts";const signer={} as any;const h=`0x${"11".repeat(32)}` as const,t=`0x${"22".repeat(32)}` as const;
test("transaction resolves only on typed finalization",async()=>{const result=await submitAndFinalize(fakeTransaction([{type:"broadcast"},{type:"best-chain",blockHash:h},{type:"finalized",blockHash:h,transactionHash:t}]),signer);assert.deepEqual(result,{success:true,value:{blockHash:h,transactionHash:t}})});test("runtime rejection and empty batch are typed failures",async()=>{const rejected=await submitAndFinalize(fakeTransaction([{type:"rejected",code:"not_authorized",message:"Denied"}]),signer);assert.equal(rejected.success,false);assert.equal(composeBatch([],()=>fakeTransaction([])).success,false)});
