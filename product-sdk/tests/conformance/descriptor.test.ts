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

import assert from "node:assert/strict"; import { createHash, generateKeyPairSync, sign } from "node:crypto";
import { readFileSync } from "node:fs"; import { resolve } from "node:path"; import test from "node:test";
import { assertMethodPayload, assertRuntimeIdentity, ProductSdkError, validateLifecycle } from "../../packages/core/src/contract.ts";
import { canonicalJson, canonicalSha256, contractDigest, ratificationStatus, REQUIRED_ROLES } from "../../packages/core/src/ratification.ts";
const repo=resolve(import.meta.dirname,"../../.."); const load=(p:string)=>JSON.parse(readFileSync(resolve(repo,p),"utf8"));
const descriptor=load("product-sdk/packages/descriptors/generated/orbis-descriptor.json"), metadataIdentity=load("docs/sdk/metadata/commons-v29.json"), vectors=load("origin/orbis/runtime/vectors/transaction-policy-v8/manifest.json");
test("descriptor binds the P5 native SDK freeze to the exact fixture and runtime versions",()=>{
 assert.equal(descriptor.kind,"cord-native-host-contract-manifest");assert.equal(descriptor.release,"origin-orbis-native-v1");assert.equal(descriptor.firstSupportedNativeSdk,false);
 assert.equal(descriptor.descriptorProvenance.methodInventory,"authoritative-typed-native-route-contract");
 assert.equal(descriptor.runtime.metadataHash,"0x50c8958f0171889a4b01093a5dd5272faf8802018f24a4ac22b231d37b2adc45");
 assert.deepEqual([descriptor.runtime.paraId,descriptor.runtime.specVersion,descriptor.runtime.transactionVersion],[1006,29,8]);
 assert.deepEqual([descriptor.currentSourceRuntime.specVersion,descriptor.currentSourceRuntime.transactionVersion],[33,8]);
 assert.equal(descriptor.currentSourceRuntime.metadataHash,"0x824731ed7cab6037cdfa3f88e40f78556f77f016136fd563acd6c7a414c29f8e");
 assert.equal(descriptor.currentSourceRuntime.metadataBoundNativeSdk,false);
 assert.equal(descriptor.fixtureIdentity.genesis_identity,"0x2584c9d420dc8160b85deaf958d776886366d5beecc2ee7b1236293d20ac70fc");
 assert.deepEqual(descriptor.networkActivation,{state:"candidate-pending",productionActivationReady:false,source:"docs/evidence/verification/p5/sdk-freeze-ratification-envelope.json"});
 assert.equal(descriptor.fixtureIdentity.status,"deterministic-clean-break-candidate-not-production-approved"); assert.equal(descriptor.productionPapiDescriptorGenerated,false);
 assert.deepEqual([descriptor.papiAvailability.runtimeMetadataCurrent,descriptor.papiAvailability.sdkAdmission],[false,false]);
 const papi=load("product-sdk/packages/descriptors/generated/commons-papi-manifest.json");assert.equal(papi.schema,"cord.commons-papi-descriptor.v1");assert.deepEqual(papi.generator,{package:"polkadot-api",version:"2.1.6"});assert.equal(papi.entry,"commons");assert.equal(papi.metadata.runtime_rfc78_hash,descriptor.runtime.metadataHash);assert.equal(papi.metadata.scale_sha256,metadataIdentity.scale_sha256);assert.ok(papi.output.length>0);assert.ok(papi.output.every((item:any)=>item.bytes>0&&/^[0-9a-f]{64}$/.test(item.sha256)));
});
test("host schema exactly freezes every descriptor native method and closed payload shape",()=>{
 const schema=load("docs/sdk/host/host-request.schema.json"),methods=descriptor.nativeHostContract.methods;
 assert.equal(descriptor.nativeHostContract.methodCount,methods.length);assert.equal(schema.oneOf.length,methods.length);
 const expected=methods.map((m:any)=>({id:`${m.capability}:${m.method}`,finality:m.finality,fields:[...m.payloadFields].sort()})).sort((a:any,b:any)=>a.id.localeCompare(b.id));
 const actual=schema.oneOf.map((b:any)=>({id:`${b.properties.capability.const}:${b.properties.method.const}`,finality:b.properties.finality.const,fields:[...b.properties.payload.required].sort()})).sort((a:any,b:any)=>a.id.localeCompare(b.id));
 assert.deepEqual(actual,expected);assert.equal(new Set(actual.map((x:any)=>x.id)).size,actual.length);for(const branch of schema.oneOf)assert.equal(branch.properties.payload.additionalProperties,false);
});
test("every current runtime vector retains frozen bytes",()=>{for(const v of vectors.files){const b=readFileSync(resolve(repo,"origin/orbis/runtime/vectors/transaction-policy-v8",v.file));assert.equal(b.length,v.bytes,v.file);assert.equal(createHash("sha256").update(b).digest("hex"),v.sha256,v.file);}});
test("every required registry ID maps to frozen evidence or an exact runtime test",()=>{const registry=load("docs/sdk/vectors/transaction-policy-vector-registry.json"),manifest=new Map(vectors.files.map((v:any)=>[`origin/orbis/runtime/vectors/transaction-policy-v8/${v.file}`,v]));assert.equal(registry.required_current_vectors.length,9);for(const item of registry.required_current_vectors){assert.match(item.runtime_test,/transaction_policy_construction_surfaces_share_the_frozen_slots/);if(!item.evidence.length)assert.match(item.expected_outcome,/no standalone SCALE fixture/);for(const ref of item.evidence){const source:any=manifest.get(ref.path);assert.ok(source,`${item.id}:${ref.path}`);assert.equal(ref.sha256,source.sha256);assert.equal(ref.expected_outcome,source.expected_error??"valid")}}});
test("ratification derives unratified and cryptographically rejects tampering",()=>{
 const env=load("docs/evidence/verification/p5/sdk-freeze-ratification-envelope.json"),initial=ratificationStatus(env),registered=new Set(env.payload.approval_policy.authorized_keys.map((key:any)=>key.role)),currentRoles=env.signatures.map((signature:any)=>signature.role);assert.equal(new Set(currentRoles).size,currentRoles.length);assert.ok(currentRoles.every((role:string)=>REQUIRED_ROLES.includes(role as any)&&registered.has(role)));assert.deepEqual(initial.verifiedRoles,[...currentRoles].sort());assert.equal(initial.p0TargetsRatified,REQUIRED_ROLES.every(role=>currentRoles.includes(role)));assert.equal(initial.productionActivationReady,false);
 for(const [name,item] of Object.entries(env.payload.contract_digests) as any){assert.equal(contractDigest(name,load(item.path)),item.canonical_sha256,name)}assert.equal(descriptor.ratificationPayloadSha256,env.payload_sha256);assert.equal(contractDigest("descriptor",descriptor),env.payload.contract_digests.descriptor.canonical_sha256);
 const {privateKey,publicKey}=generateKeyPairSync("ed25519"),der=publicKey.export({format:"der",type:"spki"}),fingerprint=createHash("sha256").update(der).digest("hex"),signed=structuredClone(env);signed.signatures=[];signed.derived_status={p0_targets_ratified:false,production_activation_ready:false};signed.payload.approval_policy.authorized_keys=[];for(const slot of signed.payload.approval_policy.approver_slots){slot.status="PENDING";slot.authorized_key_fingerprint_sha256=null}signed.payload.approval_policy.authorized_keys=[{role:"runtime-owner",fingerprint_sha256:fingerprint,public_key_spki_der_base64:der.toString("base64"),valid_from:"2026-01-01T00:00:00Z",valid_until:"2027-01-01T00:00:00Z",revoked_at:null}];signed.payload.approval_policy.approver_slots[0].status="READY";signed.payload.approval_policy.approver_slots[0].authorized_key_fingerprint_sha256=fingerprint;signed.payload_sha256=canonicalSha256(signed.payload);const payload=Buffer.from(canonicalJson(signed.payload));signed.signatures=[{role:"runtime-owner",key_fingerprint_sha256:fingerprint,payload_sha256:signed.payload_sha256,public_key_spki_der_base64:der.toString("base64"),signature_base64:sign(null,payload,privateKey).toString("base64"),signed_at:"2026-07-13T00:00:00Z"}];assert.deepEqual(ratificationStatus(signed).verifiedRoles,["runtime-owner"]);const forged=structuredClone(signed);forged.signatures[0].signature_base64=Buffer.alloc(64).toString("base64");assert.throws(()=>ratificationStatus(forged),/invalid Ed25519/);const unknownRole=structuredClone(signed);unknownRole.signatures[0].role="unknown-owner";assert.throws(()=>ratificationStatus(unknownRole),/unauthorized|misbound/);const duplicateRole=structuredClone(signed);duplicateRole.signatures.push({...duplicateRole.signatures[0]});assert.throws(()=>ratificationStatus(duplicateRole),/duplicate|misbound/);const duplicate=structuredClone(signed);duplicate.payload.approval_policy.authorized_keys.push({...duplicate.payload.approval_policy.authorized_keys[0],role:"sdk-owner"});duplicate.payload_sha256=canonicalSha256(duplicate.payload);assert.throws(()=>ratificationStatus(duplicate),/duplicate or invalid authorized key/);
});
test("lifecycle state conditionals fail closed",()=>{const h=`0x${"11".repeat(32)}`,base={version:1,intent_id:"intent-0000000001"};validateLifecycle({...base,state:"draft"});validateLifecycle({...base,state:"included",block_hash:h});validateLifecycle({...base,state:"finalized",block_hash:h,extrinsic_hash:h});validateLifecycle({...base,state:"cancelled",error:new ProductSdkError("cancelled","cancelled").toJSON()});for(const value of[{...base,state:"finalized",block_hash:h},{...base,state:"cancelled",error:new ProductSdkError("timeout","x").toJSON()},{...base,state:"draft",block_hash:h},{...base,state:"rejected"},{...base,state:"included",block_hash:12}])assert.throws(()=>validateLifecycle(value),(e:ProductSdkError)=>e.code==="invalid_input")});
test("recursive product boundary rejects case, alias, nesting and method bypasses",()=>{
 for(const payload of [{abi:[]},{ABI:[]},{AbiEncoded:"x"},{scale_payload:"x"},{contractAddress:"x"},{nested:{raw_scale:"0x"}},{nested:[{"Contract-Address":"x"}]},{subject_id:"contract ABI"}]) assert.throws(()=>assertMethodPayload("attestation","schema_by_id",payload as any),(e:ProductSdkError)=>e.code==="unsupported_surface");
 assert.throws(()=>assertMethodPayload("attestation","schema_by_id",{schema:`0x${"11".repeat(32)}`,extra:"x"}),/invalid payload/);
 assert.throws(()=>assertMethodPayload("attestation","unsupported",{schema:`0x${"11".repeat(32)}`}),(e:ProductSdkError)=>e.code==="unsupported_surface");
 assert.throws(()=>assertRuntimeIdentity(`0x${"00".repeat(32)}`,29,8,"x","x","x","candidate-pending",false,"candidate"),(e:ProductSdkError)=>e.code==="unsupported_runtime");
});
