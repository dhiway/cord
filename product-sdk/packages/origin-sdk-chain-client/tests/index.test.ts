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

import assert from "node:assert/strict";import test from "node:test";import { COMMONS_NETWORK_BINDING } from "@cord-network/origin-sdk-descriptors";import { createCommonsChainClient } from "../src/index.ts";
const provider={async finalizedBlock(){return{hash:`0x${"11".repeat(32)}` as const,number:9n}},async runtimeIdentity(){return{genesis_hash:COMMONS_NETWORK_BINDING.genesis_hash,spec_version:COMMONS_NETWORK_BINDING.spec_version,transaction_version:COMMONS_NETWORK_BINDING.transaction_version,metadata_hash:COMMONS_NETWORK_BINDING.metadata_hash,descriptor_contract_sha256:COMMONS_NETWORK_BINDING.descriptor_contract_sha256,chain_spec_source_sha256:COMMONS_NETWORK_BINDING.chain_spec_source_sha256}},async disconnect(){}};
test("all reads use the exact verified finalized hash",async()=>{const client=createCommonsChainClient(provider);let at="";const result=await client.readFinalized(async(hash)=>{at=hash;return 7});assert.equal(result.success&&result.value,7);assert.equal(at,(await provider.finalizedBlock()).hash)});test("runtime drift fails before executing a query",async()=>{const client=createCommonsChainClient({...provider,async runtimeIdentity(){return{...(await provider.runtimeIdentity()),spec_version:999}}});let called=false;const result=await client.readFinalized(async()=>{called=true});assert.equal(result.success,false);assert.equal(called,false)});
