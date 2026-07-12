#!/usr/bin/env python3
"""Verify normalized, historical and executable Orbis completion evidence v4."""
from pathlib import Path
import argparse,hashlib,json,os,re,subprocess
P=argparse.ArgumentParser(); P.add_argument('--manifest'); P.add_argument('--static',action='store_true'); a=P.parse_args()
ROOT=Path(__file__).resolve().parents[1]; MANIFEST=Path(a.manifest) if a.manifest else ROOT/'docs/orbis-completion-manifest.toml'; ZERO='0'*64
E={'meta_contract','meta_router_variant','meta_vector','meta_ingress','bulletin_v7_rehearsal','bulletin_v7_contract','provider_v8_contract','remediation_gate'}
STATELESS={'source':'excluded','package_provenance':'excluded','protocol_constant':'excluded','protocol_dependency':'excluded'}
def die(x): raise SystemExit('orbis-v4: '+x)
def sha(p): return hashlib.sha256(p.read_bytes()).hexdigest()
def fs(body): return dict(re.findall(r'^([A-Za-z0-9_]+) = "([^"]*)"$',body,re.M))
def git(*x): return subprocess.run(['git',*x],cwd=ROOT,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True,errors='ignore')
def assertion_hash(x): return hashlib.sha256(x.encode()).hexdigest()
def command_binding(cmd):
 if 'RUNTIME_METADATA_HASH=' in cmd and 'checked_in_meta_v7_fixtures' in cmd:
  return ('runtime-fixtures-compiled','origin/orbis/runtime/src/meta_v6_fixtures.rs')
 bindings=(
  ('metadata-implicit-nohash','runtime-nohash','origin/orbis/runtime/metadata-implicit-nohash/src/lib.rs'),
  ('checked_in_meta_v7_fixtures','runtime-fixtures','origin/orbis/runtime/src/meta_v6_fixtures.rs'),
  ('metadata_custom_hash_loss','runtime-custom-hash','origin/orbis/runtime/src/tests.rs'),
  ('sponsored_meta_tx_preserves','runtime-sponsored','origin/orbis/runtime/src/tests.rs'),
  ('completion_manifest_v4_evidence','runtime-manifest','origin/orbis/runtime/src/tests.rs'),
  ('conservative_weights_freeze','runtime-weights','origin/orbis/runtime/src/remediation_v3.rs'),
  ('inspector_is_fixed_stack','runtime-inspector','origin/orbis/runtime/src/remediation_v3.rs'),
  ('signed_direct_resources_payer','runtime-direct-payer','origin/orbis/runtime/src/remediation_v3.rs'),
  ('token_is_one_shot','runtime-token','origin/orbis/runtime/src/remediation_v3.rs'),
  ('pallet-bulletin-transaction-storage migration_v6_to_v7','bulletin-v6-v7','origin/orbis/pallets/transaction-storage/src/tests.rs'),
 )
 found=[(group,path) for needle,group,path in bindings if needle in cmd]
 if len(found)!=1: die('command has no unique source marker binding: '+cmd)
 return found[0]
def canonical_output(cmd,out,code):
 ansi=re.compile(r'\x1b\[[0-9;]*m'); lines=[]
 for raw in ansi.sub('',out).splitlines():
  x=raw.strip()
  if re.match(r'^(running [0-9]+ tests|test .* \.\.\. (ok|FAILED|ignored)|test result:|warning:|error:|assertion=[A-Za-z0-9_-]+:[0-9a-f]{64}$)',x):
   lines.append(re.sub(r'; finished in [0-9.]+s','; finished',x))
 return 'exit='+str(code)+'\ncommand='+cmd+'\n'+'\n'.join(sorted(set(lines)))+'\n'
text=MANIFEST.read_text(); top=fs(text.split('[[',1)[0]); audit=top.get('audited_runtime_commit',''); marker_commit=top.get('evidence_marker_commit','')
if top.get('critic_status')!='pending' or top.get('critic_evidence')!='': die('Critic must remain explicitly pending')
if not re.fullmatch('[0-9a-f]{40}',audit) or git('cat-file','-e',audit+'^{commit}').returncode: die('bad audited_runtime_commit')
if not re.fullmatch('[0-9a-f]{40}',marker_commit) or git('cat-file','-e',marker_commit+'^{commit}').returncode: die('bad evidence_marker_commit')
if git('merge-base','--is-ancestor',audit,marker_commit).returncode or git('merge-base','--is-ancestor',marker_commit,'HEAD').returncode: die('runtime baseline -> marker -> evidence commit ancestry/order violation')
equivalence_rel=top.get('nonruntime_equivalence_evidence',''); equivalence_path=ROOT/equivalence_rel
if not equivalence_rel or not equivalence_path.is_file(): die('missing marker non-runtime equivalence proof')
try: equivalence=json.loads(equivalence_path.read_text())
except Exception: die('marker non-runtime equivalence proof is not JSON')
diff_run=git('diff','--binary',audit,marker_commit,'--','origin/orbis'); changed=git('diff','--name-only',audit,marker_commit,'--','origin/orbis').stdout.splitlines()
allowed_changed=['origin/orbis/evidence_inventory_v4.rs','origin/orbis/evidence_markers_v4.rs','origin/orbis/pallets/transaction-storage/src/tests.rs','origin/orbis/runtime/metadata-implicit-nohash/README.md','origin/orbis/runtime/metadata-implicit-nohash/src/lib.rs','origin/orbis/runtime/src/meta_v6_fixtures.rs','origin/orbis/runtime/src/remediation_v3.rs','origin/orbis/runtime/src/tests.rs']
if changed!=allowed_changed or equivalence.get('changed_paths')!=allowed_changed: die('marker commit source-diff policy violation')
if equivalence.get('runtime_baseline_commit')!=audit or equivalence.get('evidence_marker_commit')!=marker_commit or equivalence.get('source_diff_sha256')!=hashlib.sha256(diff_run.stdout.encode()).hexdigest(): die('marker source-diff proof binding mismatch')
if equivalence.get('cmp_exit_code')!=0 or equivalence.get('baseline_wasm_sha256')!=equivalence.get('marker_wasm_sha256') or not re.fullmatch('[0-9a-f]{64}',equivalence.get('baseline_wasm_sha256','')) or equivalence.get('wasm_size_bytes',0)<=0: die('marker compact runtime Wasm equivalence proof mismatch')
base_lock=git('show',audit+':Cargo.lock').stdout.encode(); marker_lock=git('show',marker_commit+':Cargo.lock').stdout.encode()
if equivalence.get('cargo_lock_sha256')!=hashlib.sha256(base_lock).hexdigest() or equivalence.get('marker_cargo_lock_sha256')!=hashlib.sha256(marker_lock).hexdigest() or equivalence.get('sdk_revision')!='cc190ea8' or equivalence.get('features')!='default' or equivalence.get('build_command')!='cargo +1.93.0 build --locked --frozen -p origin-orbis-runtime' or equivalence.get('cold_target_removed_before_each_build') is not True or 'CARGO_INCREMENTAL=0' not in equivalence.get('deterministic_env','') or 'RUNTIME_METADATA_HASH' not in equivalence.get('deterministic_env','') or not equivalence.get('toolchain','').startswith('rustc 1.93.0 '): die('marker cold-build reproducibility policy mismatch')
registry_rel='origin/orbis/evidence_markers_v4.rs'; registry_current=(ROOT/registry_rel).read_text(); registry_at_marker=git('show',marker_commit+':'+registry_rel)
if registry_at_marker.returncode or registry_at_marker.stdout!=registry_current: die('source marker registry is not bound at evidence marker commit/current source')
registry={}
for group,ident,digest in re.findall(r'\("([^"]+)", "([^"]+)", "([0-9a-f]{64})"\)',registry_current):
 if ident in registry: die('duplicate source marker registry identity '+ident)
 registry[ident]=(group,digest)
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
 if '--nocapture' not in cmd: die(ident+' command does not expose source markers')
 group,marker_source=command_binding(cmd)
 historical_marker_source=git('show',marker_commit+':'+marker_source)
 marker_call='emit_evidence_markers_v4("'+group+'")'
 if historical_marker_source.returncode or marker_call not in historical_marker_source.stdout or marker_call not in (ROOT/marker_source).read_text(): die(ident+' source marker emitter is not bound at audited/current test source')
 if registry.get(ident)!=(group,r['assertion_sha256']): die(ident+' source marker registry mismatch')
 if ident=='META-EVIDENCE-NOHASH-CANNOTLOOKUP':
  exact='env -u RUNTIME_METADATA_HASH CARGO_TARGET_DIR=target/nohash cargo test --manifest-path origin/orbis/runtime/metadata-implicit-nohash/Cargo.toml --features runtime-benchmarks tests::enabled_metadata_without_compiled_hash_is_exact_cannot_lookup -- --exact --nocapture'
  readme=ROOT/'origin/orbis/runtime/metadata-implicit-nohash/README.md'
  if cmd!=exact or not readme.is_file() or exact not in readme.read_text(): die('nohash isolation command/README drift')
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
present_ids={r['id'] for _,r in evidence if r.get('status')=='present'}
if set(registry)!=present_ids: die('source marker registry evidence ID set mismatch')
# Exact source-owned inventory covers every manifest row, including stateless/excluded rows.
inventory_rel='origin/orbis/evidence_inventory_v4.rs'; inventory_current=(ROOT/inventory_rel).read_text(); inventory_at_marker=git('show',marker_commit+':'+inventory_rel)
if inventory_at_marker.returncode or inventory_at_marker.stdout!=inventory_current: die('manifest inventory is not bound at evidence marker commit/current source')
inventory_rows=re.findall(r'\("([^"]*)", "([^"]*)", "([^"]*)", "([^"]*)"\)',inventory_current)
count_match=re.search(r'MANIFEST_INVENTORY_V4_COUNT: usize = ([0-9]+);',inventory_current); digest_match=re.search(r'MANIFEST_INVENTORY_V4_BLAKE2_256: &str = "([0-9a-f]{64})";',inventory_current)
actual_inventory=sorted((table,r.get('id',r.get('name',r.get('package',r.get('revision','')))),r.get('state',''),r.get('status','')) for table,r in all_rows)
canonical_inventory=''.join('\0'.join(row)+'\n' for row in actual_inventory).encode(); inventory_digest=hashlib.blake2b(canonical_inventory,digest_size=32).hexdigest()
if not count_match or not digest_match or int(count_match.group(1))!=len(actual_inventory) or digest_match.group(1)!=inventory_digest or sorted(inventory_rows)!=actual_inventory: die('source-owned manifest inventory count/digest/identity mismatch')
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
dep=next(r for t,r in all_rows if t=='protocol_dependency' and r['id']=='DEP-Slice10-Provider')
if dep.get('requires')!='PMIG-Bulletin-V7-to-V8 provider_ref migration' or pm['PMIG-Bulletin-V7-to-V8'].get('state')!='planned': die('Slice10 provider dependency is not bound to PMIG-Bulletin-V7-to-V8')
for rel in ('docs/adr/0008-orbis-transaction-policy-pipeline.md','docs/orbis-native-capability-matrix.md'):
 d=(ROOT/rel).read_text()
 if 'PMIG-Bulletin-V6-to-V7' not in d or 'PMIG-Bulletin-V7-to-V8' not in d: die(rel+' migration IDs missing')
if next(r for t,r in evidence if r['id']=='GATE-5-EVIDENCE')['status']!='planned': die('Gate5 falsely frozen')
if not a.static:
 clean_env=os.environ.copy(); clean_env.pop('RUNTIME_METADATA_HASH',None)
 for cmd,rows in sorted(commands.items()):
  run=subprocess.run(cmd,cwd=ROOT,shell=True,text=True,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,env=clean_env)
  raw_markers=re.findall(r'(?m)^assertion=([A-Za-z0-9_-]+):([0-9a-f]{64})$',run.stdout)
  expected_markers=sorted((r['id'],r['assertion_sha256']) for r in rows)
  if sorted(raw_markers)!=expected_markers: die('raw source marker mismatch: '+cmd)
  actual=canonical_output(cmd,run.stdout,run.returncode); h=hashlib.sha256(actual.encode()).hexdigest()
  if run.returncode or rows[0]['expected_output'] not in actual or h!=rows[0]['output_sha256']: die('actual output drift: '+cmd+'\n'+run.stdout[-1200:])
report={'schema':'orbis-completion-verification-v4','manifest_sha256':sha(MANIFEST),'audited_runtime_commit':audit,'evidence_marker_commit':marker_commit,'manifest_inventory_count':len(actual_inventory),'manifest_inventory_blake2_256':inventory_digest,'present':len(normalized['present']),'planned':len(normalized['planned']),'excluded':len(normalized['excluded']),'unchecked':normalized['unchecked'],'ids_by_category':{k:sorted(v) for k,v in normalized.items()},'ids_by_table':{k:sorted(v) for k,v in sorted(by_table.items())},'row_count':len(all_rows),'evidence_present':sum(r['status']=='present' for _,r in evidence),'evidence_planned':sum(r['status']=='planned' for _,r in evidence),'commands':len(commands),'metadata_evidence':sorted(modes),'bulletin_v7':'present','provider_v8':'planned','critic_status':'pending'}
if not a.manifest:
 out=ROOT/'docs/evidence/orbis-v4/verification-report.json'; sidecar=out.parent/'verification-report.sha256'; expected=json.dumps(report,sort_keys=True,indent=2)+'\n'
 required={MANIFEST,ROOT/'Cargo.lock',ROOT/registry_rel,ROOT/inventory_rel,equivalence_path,ROOT/'scripts/prove-orbis-marker-nonruntime.sh',ROOT/'scripts/verify-orbis-completion-v4.py',out,sidecar}
 for _,r in evidence:
  if r.get('artifact_path'):
   artifact_path=ROOT/r['artifact_path']; required.add(artifact_path)
   output_path=json.loads(artifact_path.read_text()).get('output_artifact','')
   if output_path: required.add(ROOT/output_path)
  if r.get('status')=='present':
   for source_path in r.get('source_paths','').split(';'):
    if source_path.strip(): required.add(ROOT/source_path.strip())
   _,marker_source=command_binding(r['test_or_command']); required.add(ROOT/marker_source)
 for source_path in allowed_changed: required.add(ROOT/source_path)
 for p in (ROOT/'docs/evidence/orbis-v4').glob('*review*.md'): required.add(p)
 for key in ('architect_evidence','critic_evidence'):
  if top.get(key): required.add(ROOT/top[key])
 for p in sorted(required):
  try: rel=str(p.resolve().relative_to(ROOT.resolve()))
  except ValueError: die('production evidence path escapes repository: '+str(p))
  if git('ls-files','--error-unmatch','--',rel).returncode or git('diff','--quiet','HEAD','--',rel).returncode: die('production evidence is untracked or dirty: '+rel)
 evidence_status=git('status','--porcelain','--untracked-files=all','--','docs/evidence/orbis-v4')
 if evidence_status.stdout: die('production evidence directory is dirty or contains untracked files')
 if not out.is_file() or out.read_text()!=expected: die('committed verification report drift')
 expected_sidecar=hashlib.sha256(expected.encode()).hexdigest()+'  verification-report.json\n'
 if not sidecar.is_file() or sidecar.read_text()!=expected_sidecar: die('committed verification report sidecar drift')
print(json.dumps({k:v for k,v in report.items() if k not in ('ids_by_category','ids_by_table')},sort_keys=True))
