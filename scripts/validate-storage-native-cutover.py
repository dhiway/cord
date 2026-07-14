#!/usr/bin/env python3
"""Focused P4 native storage/provider cutover and alignment validator."""
from __future__ import annotations
import argparse,csv,json,re
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]

def text(path:str)->str:return (ROOT/path).read_text()
def require(ok:bool,msg:str,errors:list[str]):
 if not ok:errors.append(msg)

def main()->int:
 ap=argparse.ArgumentParser();ap.add_argument('--report',type=Path);args=ap.parse_args();errors=[]
 runtime=text('origin/orbis/runtime/src/lib.rs')
 for name,index in [('TransactionStorage',110),('HopPromotion',111),('StorageProvider',120),('Drive',121),('S3',122)]:
  require(re.search(rf'\b{name}:\s*[^=]+\s*=\s*{index}\b',runtime) is not None,f'{name} index {index} missing',errors)
 provider=text('origin/orbis/pallets/storage-provider/src/lib.rs')
 for token in ['ProviderRoots','ProviderFrontierOf','commit_provider_root','provider-root-append/v1','provider-deletion-leaf/v1','MaxDeletionProofDepth','InvalidInclusionProof','index.saturating_add(1) >= leaf_count']:
  require(token in provider,f'provider integrity token missing: {token}',errors)
 require('StorageVersion::new(5)' in provider,'provider clean-genesis schema is not v5',errors)
 api=text('origin/orbis/runtime-api/storage/src/lib.rs')
 require('RESPONSE_VERSION: u16 = 4' in api and 'fn provider_root(' in api,'storage provider API v4/root missing',errors)
 node_storage=text('origin/orbis/provider-node/src/storage.rs');node_workers=text('origin/orbis/provider-node/src/workers.rs');node_lib=text('origin/orbis/provider-node/src/lib.rs')
 require('PROTOCOL_VERSION: u16 = 4' in node_lib and all(token in node_storage for token in ['proof_leaf_count','root_frontier','root_history','root_observation']),'provider node protocol v4/history missing',errors)
 require('repair_incomplete_jsonl_tail' in node_workers,'provider JSONL torn-tail repair missing',errors)
 manifest=text('docs/orbis-completion-manifest.toml');surface_blocks=re.findall(r'\[\[node_surface\]\]\n(.*?)(?=\n\[\[|\Z)',manifest,re.S)
 provider_routes=[block for block in surface_blocks if 'kind = "provider-http-method"' in block]
 require(len(provider_routes)==21 and all('state = "present"' in block and 'evidence = "origin/orbis/provider-node/src/api.rs:handle"' in block for block in provider_routes),'manifest provider HTTP route state/evidence mismatch',errors)
 provider_workers=[block for block in surface_blocks if any(f'id = "{worker}"' in block for worker in ['WORKER-challenge_responder','WORKER-checkpoint_coordinator','WORKER-replica_sync_coordinator'])]
 require(len(provider_workers)==3 and all('state = "present"' in block and 'evidence = "origin/orbis/provider-node/src/workers.rs:run_workers"' in block for block in provider_workers),'manifest provider worker state/evidence mismatch',errors)
 rust_events=text('origin-rs/src/product_sdk/storage_events.rs');ts_events=text('product-sdk/packages/host/src/storage-events.ts')
 provider_variants=['ProviderRegistered','ProviderUpdated','ProviderStatusChanged','ProviderRemoved','Heartbeat','AgreementProposed','AgreementAccepted','AgreementCancelled','AgreementRenewalRequested','AgreementRenewed','AgreementExpired','AgreementPruned','ChallengeIssued','CheckpointSubmitted','ChallengeTimedOut','ProviderRootCommitted','DeletionAcknowledged']
 drive_variants=['DriveCreated','DriveRootUpdated','ControllerChanged','DriveTransferred','DriveArchived']
 s3_variants=['BucketCreated','ControllerChanged','BucketTransferred','BucketArchived','BucketVersioningChanged','ObjectPut','ObjectDeleted','BucketDeleted']
 for pallet,variants in [('StorageProvider',provider_variants),('Drive',drive_variants),('S3',s3_variants)]:
  for variant in variants:
   require(f'"{pallet}","{variant}"' in rust_events,f'Rust event missing {pallet}.{variant}',errors)
   require(f'{pallet}.{variant}' in ts_events,f'TS event missing {pallet}.{variant}',errors)
 hop=text('origin/orbis/pallets/hop-promotion/src/lib.rs')
 require('pub enum Event' not in hop and 'pallet_bulletin_transaction_storage::Pallet::<T>::do_store' in hop,'HopPromotion must remain internal and surface successful lifecycle through TransactionStorage.Stored',errors)
 storage_domain=text('origin-rs/src/product_sdk/domains/storage.rs');storage_transport=text('origin-rs/src/product_sdk/transport.rs')
 require('pub type StorageWrite = SubmitAndFinalize<StorageCommand>' in storage_domain and 'prepare_storage_command' in storage_transport,'TransactionStorage product writes must terminate through submit-and-finalize',errors)
 onchain='\n'.join(text(p) for p in ['origin/orbis/pallets/storage-provider/src/lib.rs','origin/orbis/pallets/drive/src/lib.rs','origin/orbis/pallets/s3/src/lib.rs'])
 require(not re.search(r'pub type (?:ContentBytes|Blob|PayloadBytes)',onchain),'duplicate on-chain content-byte ledger found',errors)
 active_paths=['origin-rs/src/product_sdk/domains/storage_provider.rs','origin-rs/src/product_sdk/domains/drive.rs','origin-rs/src/product_sdk/domains/s3.rs','origin-rs/src/product_sdk/storage_events.rs','product-sdk/src/provider.ts','product-sdk/src/drive.ts','product-sdk/src/s3.ts','product-sdk/src/storage-events.ts','product-sdk/packages/host/src/storage-events.ts']
 forbidden=re.compile(r'contract_address|contractAddress|deploymentAddress|\.sol\b|reviveContract',re.I)
 for path in active_paths:
  require(not forbidden.search(text(path)),f'stale contract/address surface in {path}',errors)
 alignment=ROOT/'docs/architecture/origin-foundation-commons-runtime-alignment.csv'
 rows=list(csv.DictReader(alignment.open(newline='')));by_id={r['id']:r for r in rows}
 require(by_id.get('NODE-PROVIDER-SERVICES',{}).get('index_or_version')=='v4','alignment provider node is not v4',errors)
 require('frontier-verified append-only' in by_id.get('GAP-PROVIDER',{}).get('disposition',''),'alignment root-proof disposition missing',errors)
 require(all(line.rstrip('\n')==line.rstrip() for line in alignment.open()),'alignment CSV has trailing whitespace',errors)
 report={'schema':'cord.storage-native-cutover.v1','scope':'P4-storage-provider-content','status':'pass' if not errors else 'fail','checks':{'runtime_indexes':[110,111,120,121,122],'provider_schema':5,'provider_node_protocol':4,'storage_provider_api':4,'manifest_provider_routes_present':21,'typed_native_event_variants':30,'transaction_storage_product_outcome':'submit-and-finalize-plus-exact-finalized-api','hop_promotion':'internal-via-transaction-storage-stored','contract_abi_address_survivors':0,'duplicate_onchain_content_ledgers':0},'errors':errors}
 if args.report:
  out=args.report if args.report.is_absolute() else ROOT/args.report;out.parent.mkdir(parents=True,exist_ok=True);out.write_text(json.dumps(report,indent=2)+'\n')
 print(json.dumps(report,sort_keys=True));return 0 if not errors else 1
if __name__=='__main__':raise SystemExit(main())
