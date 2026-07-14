#!/usr/bin/env python3
# This file is part of CORD – https://cord.network

# Copyright (C) Dhiway Networks Pvt. Ltd.
# SPDX-License-Identifier: GPL-3.0-or-later

# CORD is free software: you can redistribute it and/or modify
# it under the terms of the GNU General Public License as published by
# the Free Software Foundation, either version 3 of the License, or
# (at your option) any later version.

# CORD is distributed in the hope that it will be useful,
# but WITHOUT ANY WARRANTY; without even the implied warranty of
# MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
# GNU General Public License for more details.

# You should have received a copy of the GNU General Public License
# along with CORD. If not, see <https://www.gnu.org/licenses/>.

"""Validate the P0 architecture/SDK/host/EQC contracts without claiming runtime implementation."""
from pathlib import Path
import argparse, hashlib, json, subprocess, sys
ROOT=Path(__file__).resolve().parents[4]
FAIL=[]; BLOCK=[]; DOWNSTREAM=[]; CHECK=[]
def load(rel):
 p=ROOT/rel
 try: obj=json.loads(p.read_text()); CHECK.append(f"json:{rel}"); return obj
 except Exception as e: FAIL.append(f"{rel}: {e}"); return {}
def eq(label,a,b):
 if a!=b: FAIL.append(f"{label}: {a!r} != {b!r}")
 else: CHECK.append(label)
def require(label,cond):
 (CHECK if cond else FAIL).append(label if cond else f"missing/invalid {label}")
def run(label,command):
 result=subprocess.run(command,cwd=ROOT,text=True,capture_output=True)
 if result.returncode: FAIL.append(f"{label}: exit {result.returncode}: {(result.stdout+result.stderr).strip()}")
 else: CHECK.append(f"command:{label}")

def main():
 ap=argparse.ArgumentParser(); ap.add_argument('--write-report',action='store_true'); args=ap.parse_args()
 branch=subprocess.check_output(['git','branch','--show-current'],cwd=ROOT,text=True).strip()
 head=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip()
 eq('branch',branch,'sm-update-sub-0x63'); eq('head',head,'439a1b62da11175129ad5390186230a64d569810')
 compat=load('docs/sdk/compatibility-manifest.json'); ext=load('docs/sdk/signed-extension-manifest.json')
 slo=load('docs/evidence/performance/service-slo-manifest.json'); host=load('docs/sdk/host/fake-host-scenarios.json')
 rat=load('docs/evidence/verification/p0/ratification-envelope.json')
 eq('para_id',compat.get('network',{}).get('para_id'),1006); eq('orbis spec',compat.get('network',{}).get('orbis_spec_version'),29); eq('orbis tx',compat.get('network',{}).get('orbis_transaction_version'),8)
 source_hash=hashlib.sha256((ROOT/'origin/orbis/runtime/src/lib.rs').read_bytes()).hexdigest(); eq('runtime source hash',compat.get('runtime_source',{}).get('runtime_source_sha256'),source_hash); eq('extension source hash',ext.get('runtime',{}).get('source_sha256'),source_hash)
 payload_hash=hashlib.sha256(json.dumps(rat.get('payload',{}),sort_keys=True,separators=(',',':')).encode()).hexdigest(); eq('ratification payload hash',rat.get('payload_sha256'),payload_hash)
 require('ratification targets-only no-performance scope',rat.get('payload',{}).get('scope')=='p0-targets-and-client-contracts-only' and rat.get('payload',{}).get('performance_claim') is False)
 roles=rat.get('payload',{}).get('approval_policy',{}).get('required_roles',[]); require('ratification required roles',roles==['runtime-owner','sdk-owner','security-owner','performance-owner','architecture-owner'] and len(set(roles))==5)
 slots=rat.get('payload',{}).get('approval_policy',{}).get('approver_slots',[]); keys=rat.get('payload',{}).get('approval_policy',{}).get('authorized_keys',[])
 require('tracker-backed ready key slots',len(slots)==5 and len(keys)==5 and all(x.get('status')=='READY' and isinstance(x.get('authorized_key_fingerprint_sha256'),str) and x.get('tracker','').startswith('P0-APPROVER-') for x in slots) and len({x.get('fingerprint_sha256') for x in keys})==5 and len({x.get('public_key_spki_der_base64') for x in keys})==5)
 eq('P0 fixture identity',rat.get('payload',{}).get('fixture_identity',{}).get('genesis_identity'),'0x40519e2e0e894e9b68defd9df6697619116ea693d0864913c98b5c65cd4e511a')
 eq('chain-spec source hash',rat.get('payload',{}).get('fixture_identity',{}).get('chain_spec_source_sha256'),hashlib.sha256((ROOT/'origin/orbis/node/src/chain_spec.rs').read_bytes()).hexdigest())
 for name,manifest in [('compatibility',compat),('extensions',ext),('SLO',slo)]:
  eq(f'{name} canonical ratification hash',manifest.get('ratification',{}).get('payload_sha256'),payload_hash)
  eq(f'{name} P0 target status',manifest.get('ratification',{}).get('p0_targets_status'),'RATIFIED')
 descriptor=load('product-sdk/packages/descriptors/generated/orbis-descriptor.json'); eq('descriptor payload binding',descriptor.get('ratificationPayloadSha256'),payload_hash)
 def canonical_digest(name,rel):
  value=load(rel)
  if name in ('compatibility','extensions','slo'): value.pop('ratification',None)
  if name=='descriptor': value.pop('ratificationPayloadSha256',None)
  return hashlib.sha256(json.dumps(value,sort_keys=True,separators=(',',':')).encode()).hexdigest()
 contract_paths={'compatibility':'docs/sdk/compatibility-manifest.json','extensions':'docs/sdk/signed-extension-manifest.json','slo':'docs/evidence/performance/service-slo-manifest.json','host':'docs/sdk/host/host-request.schema.json','error':'docs/sdk/native-error.schema.json','lifecycle':'docs/sdk/native-lifecycle.schema.json','vectors':'docs/sdk/vectors/transaction-policy-vector-registry.json','descriptor':'product-sdk/packages/descriptors/generated/orbis-descriptor.json','test_report':'docs/evidence/verification/p0/test-report-contract.json'}
 for name,rel in contract_paths.items(): eq(f'ratification contract digest {name}',rat.get('payload',{}).get('contract_digests',{}).get(name,{}).get('canonical_sha256'),canonical_digest(name,rel))
 expected_normal=['AsPerson','ScoreAsParticipant','PeopleLiteAuth','AsResources','HonourAuth','AuthorizeCall','CheckNonZeroSender','CheckSpecVersion','CheckTxVersion','CheckGenesis','CheckMortality','CheckNonce','CheckWeight','ChargeAssetTxPayment','ValidateStorageCalls','CheckMetadataHash','EthSetOrigin','StorageWeightReclaim']
 expected_meta=['VerifyMultiSignature','ConsumePaidMetaIngressV7','MetaTxMarker','CheckNonZeroSender','CheckSpecVersion','CheckTxVersion','CheckGenesis','CheckMortality','CheckNonce','ScoreAsParticipant','MetaAccountBoundPoliciesV6','HonourAuth','ValidateStorageCalls','CheckMetadataHash']
 for surface in ('normal','ethereum','authorized'): eq(f'{surface} extension order',ext.get('surfaces',{}).get(surface,{}).get('extensions'),expected_normal)
 eq('meta extension order',ext.get('surfaces',{}).get('meta_inner',{}).get('extensions'),expected_meta)
 eq('E mix',sum(slo.get('E',{}).get('mix_percent',{}).values()),100); eq('Q mix',sum(slo.get('Q',{}).get('mix_percent',{}).values()),100); eq('C mix',sum(slo.get('C',{}).get('mix_percent',{}).values()),100); eq('C bucket mix',sum(x['proportion_percent'] for x in slo.get('C',{}).get('content_buckets',[])),100)
 for key in ('E','Q','C'): require(f'{key} numeric targets',all(not isinstance(v,(dict,list)) or v for v in slo.get(key,{}).values()))
 require('host hostile scenarios',len(host.get('scenarios',[]))>=10)
 required_sections=['## Context','## Decision','## Drivers','## Alternatives','## Authority and security','## Data/API compatibility','## Consequences','## Verification','## Reversal','## Follow-ups']
 for n in range(9,16):
  files=list((ROOT/'docs/adr').glob(f'{n:04d}-*.md')); require(f'ADR {n:04d} unique',len(files)==1)
  if files:
   text=files[0].read_text()
   for sec in required_sections: require(f'{files[0].name}:{sec}',sec in text)
 for p in sorted((ROOT/'docs/architecture/domains').glob('*.md')):
  if p.name=='README.md': continue
  table_rows=[line for line in p.read_text().splitlines() if line.startswith('| ') and line.split('|')[1].strip().isdigit()]
  eq(f'{p.name} 21 rows',len(table_rows),21); require(f'{p.name} no TBD/unknown','tbd' not in p.read_text().lower())
 run('canonical cryptographic ratification',['npm','--prefix','product-sdk','run','validate:ratification'])
 if not rat.get('derived_status',{}).get('p0_targets_ratified'): BLOCK.append('canonical P0 target envelope is cryptographically ready but PENDING five tracker-backed external role keys/signatures')
 if not rat.get('derived_status',{}).get('production_activation_ready'): DOWNSTREAM.append('production activation remains separately BLOCKED on final genesis and campaign authorization')
 eq('Rust exact guard status',compat.get('supported',{}).get('rust',{}).get('state'),'target-pending-exact-runtime-identity-guard')
 require('lifecycle state conditionals',len(load('docs/sdk/native-lifecycle.schema.json').get('allOf',[]))>=5)
 require('Product SDK npm lockfile',(ROOT/'product-sdk/package-lock.json').is_file())
 phase_index=load('docs/evidence/verification/p0/index.json')
 commands=phase_index.get('commands',{})
 expected_commands={
  'canonical_ratification':'npm --prefix product-sdk run validate:ratification',
  'typescript_descriptor_conformance':'npm --prefix product-sdk test',
  'fake_host_conformance':'npm --prefix product-sdk run test:host',
  'eqc_client_validation':'npm --prefix product-sdk run validate:eqc',
 }
 for name,command in expected_commands.items(): eq(f'{name} frozen command',commands.get(name,{}).get('command'),command)
 run('Product SDK complete P0 suite',['npm','--prefix','product-sdk','test'])
 result={'schema_version':1,'scope':'p0-architecture-sdk-host-eqc-contracts','pass':not FAIL and not BLOCK,'contract_validation_pass':not FAIL,'gate_status':'blocked' if BLOCK else ('pass' if not FAIL else 'fail'),'checks':CHECK,'failures':FAIL,'blockers':BLOCK,'downstream_blockers':DOWNSTREAM}
 if args.write_report:
  (ROOT/'docs/evidence/verification/p0/contracts-report.json').write_text(json.dumps(result,indent=2,sort_keys=True)+'\n')
 print(json.dumps(result,indent=2,sort_keys=True))
 return 0 if not FAIL else 1
if __name__=='__main__': sys.exit(main())
