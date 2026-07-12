#!/usr/bin/env python3
"""Normalize and verify checked Orbis completion evidence v4 without third-party TOML modules."""
from pathlib import Path
import hashlib,json,re,subprocess,sys
ROOT=Path(__file__).resolve().parents[1]
MANIFEST=ROOT/'docs/orbis-completion-manifest.toml'
ZERO='0'*64
EVIDENCE_TABLES={'meta_contract','meta_router_variant','meta_vector','meta_ingress','bulletin_v7_rehearsal','bulletin_v7_contract','provider_v8_contract','remediation_gate'}

def die(msg): raise SystemExit('orbis-v4: '+msg)
def sha(path): return hashlib.sha256(path.read_bytes()).hexdigest()
def fields(body):
 out={}
 for key,val in re.findall(r'^([A-Za-z0-9_]+) = "([^"]*)"$',body,re.M): out[key]=val
 return out
text=MANIFEST.read_text()
if not re.search(r'^manifest_version = 4$',text,re.M): die('manifest_version is not 4')
if 'implemented-pending-evidence' in text: die('pending-evidence status is forbidden in v4')
rows=[]
inventory_present=0
for table,body in re.findall(r'^\[\[([^]]+)\]\]\n(.*?)(?=^\[\[|\Z)',text,re.M|re.S):
 all_row=fields(body)
 if all_row.get('state','').startswith('present'):
  inventory_present += 1
  if not all_row.get('evidence'): die(f'{table}.{all_row.get("id",all_row.get("name","?"))} present inventory lacks evidence')
 if table not in EVIDENCE_TABLES: continue
 row=all_row; row['_table']=table; rows.append(row)
 required=('id','source_paths','source_symbol','test_or_command','expected_assertion','artifact_path','artifact_sha256','source_commit','status')
 for key in required:
  if not row.get(key): die(f'{table}.{row.get("id","?")} missing {key}')
 status=row['status']
 if status=='present':
  if row['artifact_sha256']==ZERO: die(f'{row["id"]} has zero SHA')
  try: subprocess.check_call(['git','cat-file','-e',row['source_commit']+'^{commit}'],cwd=ROOT,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
  except subprocess.CalledProcessError: die(f'{row["id"]} bad commit {row["source_commit"]}')
  source_text=''
  for item in row['source_paths'].split(';'):
   path=ROOT/item.strip()
   if not path.is_file(): die(f'{row["id"]} missing source path {item.strip()}')
   try: source_text+=path.read_text(errors='ignore')
   except Exception: pass
  if row['source_symbol'] not in source_text: die(f'{row["id"]} symbol {row["source_symbol"]} absent')
  if not re.search(r'(^| )(cargo|bash|python3|origin/|scripts/)',row['test_or_command']): die(f'{row["id"]} command is not reproducible')
  artifact=ROOT/row['artifact_path']
  if not artifact.is_file(): die(f'{row["id"]} missing artifact')
  actual=sha(artifact)
  if actual!=row['artifact_sha256']: die(f'{row["id"]} artifact SHA mismatch {actual}')
 elif status=='planned':
  if row['artifact_sha256']!=ZERO: die(f'{row["id"]} planned row has fake evidence')
 else: die(f'{row["id"]} invalid status {status}')
# Migration boundary: completed reverse-index V6->V7 is evidence-backed; provider V7->V8 stays future.
if not all(r['status']=='present' for r in rows if r['_table'] in ('bulletin_v7_rehearsal','bulletin_v7_contract')): die('Bulletin V7 repair not fully present')
if not all(r['status']=='planned' for r in rows if r['_table']=='provider_v8_contract'): die('provider V8 must remain planned')
migrations=(ROOT/'origin/orbis/pallets/transaction-storage/src/migrations.rs').read_text()
if 'MigrateV6ToV7' not in migrations or 'StorageVersion::new(7)' not in migrations: die('V7 migration source missing')
if 'MigrateV7ToV8' in migrations: die('planned provider V8 was falsely implemented')
ids={r['id'] for r in rows}
metadata_ids={'META-EVIDENCE-COMPILED-ENABLED','META-EVIDENCE-CUSTOM-LOSS','META-EVIDENCE-NOHASH-CANNOTLOOKUP','META-EVIDENCE-EARLY-PROPAGATION'}
if not metadata_ids <= ids: die('four-way metadata evidence incomplete')
report={'schema':'orbis-completion-verification-v4','manifest_sha256':sha(MANIFEST),'present':sum(r['status']=='present' for r in rows),'planned':sum(r['status']=='planned' for r in rows),'zero_sha':sum(r['artifact_sha256']==ZERO for r in rows),'inventory_present':inventory_present,'artifact_count':sum(r['status']=='present' for r in rows),'metadata_evidence':sorted(metadata_ids),'bulletin_v7':'present','provider_v8':'planned'}
out=ROOT/'docs/evidence/orbis-v4/verification-report.json'; out.write_text(json.dumps(report,sort_keys=True,indent=2)+'\n')
(ROOT/'docs/evidence/orbis-v4/verification-report.sha256').write_text(sha(out)+'  verification-report.json\n')
print(json.dumps(report,sort_keys=True))
