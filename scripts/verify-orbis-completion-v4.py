#!/usr/bin/env python3
"""Verify normalized, historical and executable Orbis completion evidence v4."""
from pathlib import Path
import argparse,hashlib,json,re,subprocess
P=argparse.ArgumentParser(); P.add_argument('--manifest'); P.add_argument('--static',action='store_true'); a=P.parse_args()
ROOT=Path(__file__).resolve().parents[1]; MANIFEST=Path(a.manifest) if a.manifest else ROOT/'docs/orbis-completion-manifest.toml'; ZERO='0'*64
E={'meta_contract','meta_router_variant','meta_vector','meta_ingress','bulletin_v7_rehearsal','bulletin_v7_contract','provider_v8_contract','remediation_gate'}
STATELESS={'source':'excluded','package_provenance':'excluded','protocol_constant':'excluded','protocol_dependency':'excluded'}
def die(x): raise SystemExit('orbis-v4: '+x)
def sha(p): return hashlib.sha256(p.read_bytes()).hexdigest()
def fs(body): return dict(re.findall(r'^([A-Za-z0-9_]+) = "([^"]*)"$',body,re.M))
def git(*x): return subprocess.run(['git',*x],cwd=ROOT,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True,errors='ignore')
def assertion_hash(x): return hashlib.sha256(x.encode()).hexdigest()
def canonical_output(cmd,out,code,rows):
 ansi=re.compile(r'\x1b\[[0-9;]*m'); lines=[]
 for raw in ansi.sub('',out).splitlines():
  x=raw.strip()
  if re.match(r'^(running [0-9]+ tests|test .* \.\.\. (ok|FAILED|ignored)|test result:|warning:|error:)',x):
   lines.append(re.sub(r'; finished in [0-9.]+s','; finished',x))
 markers=['assertion='+r['id']+':'+assertion_hash(r['expected_assertion']) for r in sorted(rows,key=lambda z:z['id'])]
 return 'exit='+str(code)+'\ncommand='+cmd+'\n'+'\n'.join(sorted(set(lines))+markers)+'\n'
text=MANIFEST.read_text(); top=fs(text.split('[[',1)[0]); audit=top.get('audited_runtime_commit','')
if top.get('critic_status')!='pending' or top.get('critic_evidence')!='': die('Critic must remain explicitly pending')
if not re.fullmatch('[0-9a-f]{40}',audit) or git('cat-file','-e',audit+'^{commit}').returncode: die('bad audited_runtime_commit')
if 'implemented-pending-evidence' in text: die('implemented-pending-evidence forbidden')
normalized={'present':[],'planned':[],'excluded':[],'unchecked':[]}; evidence=[]; all_rows=[]; seen=set(); by_table={}
for table,body in re.findall(r'^\[\[([^]]+)\]\]\n(.*?)(?=^\[\[|\Z)',text,re.M|re.S):
 r=fs(body); r['_body']=body; ident=r.get('id',r.get('name',r.get('package',r.get('revision','')))); identity=table+':'+ident
 if not ident: normalized['unchecked'].append(table+':<missing-id>'); continue
 if identity in seen: die('duplicate normalized identity '+identity)
 seen.add(identity); all_rows.append((table,r)); by_table.setdefault(table,[]).append(identity)
 state=r.get('state',''); status=r.get('status','')
 if table in E: category=status if status in ('present','planned') else 'unchecked'; evidence.append((table,r))
 elif state.startswith('present'): category='present'
 elif state.startswith(('planned','pending')): category='planned'
 elif state.startswith(('excluded','reserved')): category='excluded'
 elif not state and not status and table in STATELESS: category=STATELESS[table]
 else: category='unchecked'
 if state.startswith('present') and status=='planned' or state.startswith(('planned','pending')) and status=='present': die(identity+' state/status contradiction')
 normalized[category].append(identity)
if sum(map(len,normalized.values()))!=len(all_rows) or set().union(*map(set,normalized.values()))!=seen: die('normalized union is not exactly all rows')
if normalized['unchecked']: die('unchecked rows: '+','.join(normalized['unchecked']))
commands={}
for table,r in evidence:
 ident=r['id']; status=r['status']
 if status=='planned':
  for k in ('source_paths','source_symbol','test_or_command','expected_assertion','artifact_path','artifact_sha256','source_commit','expected_output','output_sha256'):
   if r.get(k,''): die(ident+' planned row has nonblank '+k)
  if not r.get('planned_slice') or not r.get('dependency_ids'): die(ident+' incomplete planned schema')
  continue
 for k in ('source_paths','source_symbol','test_or_command','expected_assertion','artifact_path','artifact_sha256','source_commit','expected_output','output_sha256','assertion_sha256'):
  if not r.get(k): die(ident+' missing '+k)
 if assertion_hash(r['expected_assertion'])!=r['assertion_sha256']: die(ident+' assertion mismatch')
 commit=r['source_commit']
 if not re.fullmatch('[0-9a-f]{40}',commit) or git('merge-base','--is-ancestor',commit,audit).returncode: die(ident+' source commit not audited ancestor')
 historical=''
 for rel in r['source_paths'].split(';'):
  shown=git('show',commit+':'+rel.strip())
  if shown.returncode: die(ident+' historical path missing: '+rel.strip())
  historical+=shown.stdout
 if r['source_symbol'] not in historical: die(ident+' historical symbol absent')
 cmd=r['test_or_command']
 if 'cargo test ' not in cmd or ('CARGO_TARGET_DIR=target/evidence-v4' not in cmd and ident!='META-EVIDENCE-NOHASH-CANNOTLOOKUP'): die(ident+' command not isolated')
 if re.search(r'--lib (remediation|completion_manifest)($| )',cmd): die(ident+' broad non-mapping command')
 if ident=='META-EVIDENCE-NOHASH-CANNOTLOOKUP':
  exact='env -u RUNTIME_METADATA_HASH CARGO_TARGET_DIR=target/nohash cargo test --manifest-path origin/orbis/runtime/metadata-implicit-nohash/Cargo.toml --features runtime-benchmarks tests::enabled_metadata_without_compiled_hash_is_exact_cannot_lookup -- --exact'
  if cmd!=exact or not (ROOT/'origin/orbis/runtime/metadata-implicit-nohash/README.md').is_file(): die('nohash isolation command/README drift')
 artifact=ROOT/r['artifact_path']
 if r['artifact_sha256']==ZERO or not artifact.is_file() or sha(artifact)!=r['artifact_sha256']: die(ident+' artifact missing/zero/hash mismatch')
 try: data=json.loads(artifact.read_text())
 except Exception: die(ident+' artifact not normalized JSON')
 for k,want in [('id',ident),('source_commit',commit),('source_paths',r['source_paths']),('source_symbol',r['source_symbol']),('command',cmd),('expected',r['expected_assertion']),('expected_output',r['expected_output']),('output_sha256',r['output_sha256']),('assertion_sha256',r['assertion_sha256'])]:
  if data.get(k)!=want: die(ident+' artifact binding mismatch '+k)
 op=ROOT/data.get('output_artifact','')
 if not op.is_file() or sha(op)!=data.get('output_artifact_sha256') or sha(op)!=r['output_sha256']: die(ident+' actual output artifact drift')
 stored=op.read_text()
 if 'assertion='+ident+':'+r['assertion_sha256'] not in stored: die(ident+' unmatched expected_assertion marker')
 commands.setdefault(cmd,[]).append(r)
# Exact metadata kind equality, not subset/intersection.
modes={'META-EVIDENCE-COMPILED-ENABLED','META-EVIDENCE-CUSTOM-LOSS','META-EVIDENCE-NOHASH-CANNOTLOOKUP','META-EVIDENCE-EARLY-PROPAGATION'}
actual_modes={r['id'] for t,r in evidence if t=='meta_contract' and r.get('kind')=='metadata-evidence'}
if actual_modes!=modes: die('metadata evidence ID set mismatch')
# Migration identities and cross-document lifecycle truth.
pm={r['id']:r for t,r in all_rows if t=='protocol_migration'}
if set(pm)!={'PMIG-Bulletin-V5-to-V6','PMIG-Bulletin-V6-to-V7','PMIG-Bulletin-V7-to-V8'}: die('protocol migration identity enumeration mismatch')
# integer values are not captured by fs: prove exact text in row source.
if not re.search(r'^from_version = 6$.*^to_version = 7$.*^owner = "slice-1"$.*^state = "present"$',pm['PMIG-Bulletin-V6-to-V7']['_body'],re.M|re.S): die('V6-to-V7 migration conflated')
if not re.search(r'^from_version = 7$.*^to_version = 8$.*^owner = "slice-10"$.*^state = "planned"$',pm['PMIG-Bulletin-V7-to-V8']['_body'],re.M|re.S): die('V7-to-V8 provider plan conflated')
for rel in ('docs/adr/0008-orbis-transaction-policy-pipeline.md','docs/orbis-native-capability-matrix.md'):
 d=(ROOT/rel).read_text()
 if 'PMIG-Bulletin-V6-to-V7' not in d or 'PMIG-Bulletin-V7-to-V8' not in d: die(rel+' migration IDs missing')
if next(r for t,r in evidence if r['id']=='GATE-5-EVIDENCE')['status']!='planned': die('Gate5 falsely frozen')
if not a.static:
 clean_env=__import__('os').environ.copy(); clean_env.pop('RUNTIME_METADATA_HASH',None)
 for cmd,rows in sorted(commands.items()):
  run=subprocess.run(cmd,cwd=ROOT,shell=True,text=True,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,env=clean_env)
  actual=canonical_output(cmd,run.stdout,run.returncode,rows); h=hashlib.sha256(actual.encode()).hexdigest()
  if run.returncode or rows[0]['expected_output'] not in actual or h!=rows[0]['output_sha256']: die('actual output drift: '+cmd+'\n'+run.stdout[-1200:])
report={'schema':'orbis-completion-verification-v4','manifest_sha256':sha(MANIFEST),'audited_runtime_commit':audit,'present':len(normalized['present']),'planned':len(normalized['planned']),'excluded':len(normalized['excluded']),'unchecked':normalized['unchecked'],'ids_by_category':{k:sorted(v) for k,v in normalized.items()},'ids_by_table':{k:sorted(v) for k,v in sorted(by_table.items())},'row_count':len(all_rows),'evidence_present':sum(r['status']=='present' for _,r in evidence),'evidence_planned':sum(r['status']=='planned' for _,r in evidence),'commands':len(commands),'metadata_evidence':sorted(modes),'bulletin_v7':'present','provider_v8':'planned','critic_status':'pending'}
if not a.manifest:
 out=ROOT/'docs/evidence/orbis-v4/verification-report.json'; out.write_text(json.dumps(report,sort_keys=True,indent=2)+'\n'); (out.parent/'verification-report.sha256').write_text(sha(out)+'  verification-report.json\n')
print(json.dumps({k:v for k,v in report.items() if k not in ('ids_by_category','ids_by_table')},sort_keys=True))
