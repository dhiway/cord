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


import assert from "node:assert/strict";import test from "node:test";
import {createCommonsChainClient} from "@cord-network/origin-sdk-chain-client";import {COMMONS_NETWORK_BINDING} from "@cord-network/origin-sdk-descriptors";
import {createFakeReviveRuntime} from "../src/testing.ts";import {createReviveContractClient,defineAppContract,reviveAddress,selector} from "../src/index.ts";
const codec={encode:(value:number)=>Uint8Array.of(value),decode:(bytes:Uint8Array)=>bytes[0]??0};
const binding={genesis_hash:COMMONS_NETWORK_BINDING.genesis_hash,spec_version:COMMONS_NETWORK_BINDING.spec_version,transaction_version:COMMONS_NETWORK_BINDING.transaction_version,metadata_hash:COMMONS_NETWORK_BINDING.metadata_hash,descriptor_contract_sha256:COMMONS_NETWORK_BINDING.descriptor_contract_sha256,chain_spec_source_sha256:COMMONS_NETWORK_BINDING.chain_spec_source_sha256};
test("optional Revive client queries app-owned logic and rejects native authority",async()=>{const fake=createFakeReviveRuntime({query:{reverted:false,data:Uint8Array.of(7),gasRequired:9n,storageDeposit:0n}});const chain=createCommonsChainClient({finalizedBlock:async()=>({hash:`0x${"11".repeat(32)}`,number:1n}),runtimeIdentity:async()=>binding,disconnect:async()=>{}});const message={name:"score",selector:selector("0x01020304"),mutates:false,input:codec,output:codec} as const;defineAppContract({name:"Festival Scores",domain:"festival-scores",messages:{score:message}});const result=await createReviveContractClient(chain,fake.runtime).query(reviveAddress(`0x${"22".repeat(20)}`),message,3);assert.equal(result.success&&result.value.value,7);assert.throws(()=>defineAppContract({name:"Duplicate Names",domain:"names",messages:{score:message}}),/canonical native Commons authority/)});
