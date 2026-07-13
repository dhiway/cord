#!/usr/bin/env python3
"""Verify the bounded Orbis Slice 2 manifest-v5 transition in pending or designed closure phase."""
from pathlib import Path
import argparse,hashlib,json,os,re,subprocess,sys,tempfile
P=argparse.ArgumentParser();P.add_argument('--manifest');P.add_argument('--static',action='store_true');P.add_argument('--write-report',action='store_true');P.add_argument('--closure-schema',action='store_true');a=P.parse_args()
ROOT=Path(__file__).resolve().parents[1]; MANIFEST=Path(a.manifest) if a.manifest else ROOT/'docs/orbis-completion-manifest.toml'
if a.closure_schema and os.environ.get('ORBIS_V5_SYNTHETIC_CLOSURE')!='1':
 result=subprocess.run([sys.executable,str(ROOT/'scripts/test-orbis-v5-closure-schema.py')],cwd=ROOT)
 raise SystemExit(result.returncode)
A='317a3a5b3dda0964958e08b94c683b1cc1b8e387';M='ce60319f85a13b29e143a4d15189616f38abe777';ZERO='0'*64
E={'meta_contract','meta_router_variant','meta_vector','meta_ingress','bulletin_v7_rehearsal','bulletin_v7_contract','provider_v8_contract','remediation_gate','slice2_evidence'}
STATELESS={'source':'excluded','package_provenance':'excluded','protocol_constant':'excluded','protocol_dependency':'excluded'}
CLOSURE_DOCS_ONLY_ALLOWLIST=[
	'docs/evidence/orbis-v5/GATE-6-SLICE2-EVIDENCE.json',
	'docs/evidence/orbis-v5/GATE-6-SLICE2-EVIDENCE.out',
	'docs/evidence/orbis-v5/architect-evidence-review-clear.md',
 'docs/evidence/orbis-v5/critic-evidence-review-clear.md',
 'docs/evidence/orbis-v5/verification-report.json',
 'docs/evidence/orbis-v5/verification-report.sha256',
 'docs/orbis-completion-manifest.toml',
]
def die(x):raise SystemExit('orbis-v5: '+x)
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def git(*x):return subprocess.run(['git',*x],cwd=ROOT,text=True,stdout=subprocess.PIPE,stderr=subprocess.PIPE,errors='ignore')
def fs(body):return dict(re.findall(r'^([A-Za-z0-9_]+) = "([^"]*)"$',body,re.M))
def parse(text):
 rows=[]
 for table,body in re.findall(r'^\[\[([^]]+)\]\]\n(.*?)(?=^\[\[|\Z)',text,re.M|re.S):
  r=fs(body);r['_body']=body;ident=r.get('id',r.get('name',r.get('package',r.get('revision',''))));
  if not ident:die('missing row identity in '+table)
  rows.append((table,ident,r))
 return rows
def canonical(cmd,out,code):
 lines=[]
 for raw in re.sub(r'\x1b\[[0-9;]*m','',out).splitlines():
  x=raw.strip()
  if re.match(r'^(running [0-9]+ tests|test result:|assertion=[A-Za-z0-9_-]+:[0-9a-f]{64}$)',x):lines.append(re.sub(r'; finished in [0-9.]+s','; finished',x))
 return 'exit='+str(code)+'\ncommand='+cmd+'\n'+'\n'.join(sorted(set(lines)))+'\n'
text=MANIFEST.read_text();top=fs(text.split('[[',1)[0]);rows=parse(text);lookup={(t,i):r for t,i,r in rows}
if top.get('manifest_version') is not None: pass
if not re.search(r'^manifest_version = 5$',text,re.M):die('manifest_version is not 5')
closed_phase=top.get('evidence_phase')=='closed'
for k,v in [('frozen_at_commit',A),('audited_runtime_commit',A),('evidence_marker_commit',M),('transition_architect_status','clear'),('transition_critic_status','clear'),('product_architect_status','clear'),('product_critic_status','clear')]:
 if top.get(k)!=v:die('transition field mismatch '+k)
if closed_phase:
 for k,v in [('evidence_architect_status','clear'),('evidence_critic_status','clear'),('evidence_architect_evidence','docs/evidence/orbis-v5/architect-evidence-review-clear.md'),('evidence_critic_evidence','docs/evidence/orbis-v5/critic-evidence-review-clear.md')]:
  if top.get(k)!=v:die('closed review field mismatch '+k)
 pending_commit=top.get('pending_evidence_commit','')
 if not re.fullmatch(r'[0-9a-f]{40}',pending_commit) or git('merge-base','--is-ancestor',pending_commit,'HEAD').returncode:die('closed pending_evidence_commit is not an ancestor')
else:
 if top.get('evidence_phase')!='pending-review' or top.get('pending_evidence_commit',''):die('pending phase/commit schema drift')
 for k,v in [('evidence_architect_status','pending'),('evidence_critic_status','pending'),('evidence_architect_evidence',''),('evidence_critic_evidence','')]:
  if top.get(k)!=v:die('pending review field mismatch '+k)
if git('merge-base','--is-ancestor',A,M).returncode or git('merge-base','--is-ancestor',M,'HEAD').returncode:die('strict A -> M -> HEAD ancestry failure')
# Historical v4 is executable and immutable.
if os.environ.get('ORBIS_V5_SKIP_V4') == '1':
 pass
elif not a.static:
 r=subprocess.run([sys.executable,str(ROOT/'scripts/verify-orbis-completion-v4.py')],cwd=ROOT)
 if r.returncode:die('historical v4 full wrapper failed')
else:
 r=subprocess.run([sys.executable,str(ROOT/'scripts/verify-orbis-completion-v4.py'),'--static'],cwd=ROOT)
 if r.returncode:die('historical v4 static wrapper failed')
base_text=git('show',A+':docs/orbis-completion-manifest.toml').stdout;base=parse(base_text)
bmap={(t,i):r for t,i,r in base};cmap=lookup
sub={'MIG-indiv-pallet-score':'MIG-pallet-orbis-score','MIG-indiv-pallet-honour':'MIG-pallet-orbis-honour'}
changed={('runtime_pallet','PAL-097'),('runtime_pallet','PAL-099'),('benchmark','BENCH-41'),('benchmark','BENCH-43')}
adds={('slice2_evidence',x) for x in ('S2-SURFACES-01','S2-FIXTURES-01','S2-PAYOUT-01','S2-BENCHMARK-01','S2-MIGRATIONS-01')}|{('remediation_gate','GATE-6-SLICE2-EVIDENCE')}
for key,r in bmap.items():
 if key[0]=='migration' and key[1] in sub:
  if ('migration',sub[key[1]]) not in cmap:die('migration substitution missing '+key[1])
 elif key in changed:pass
 elif key not in cmap or cmap[key]['_body'].rstrip()!=r['_body'].rstrip():die('unapproved inherited row body drift '+str(key))
expected_keys=(set(bmap)-changed-{('migration',x) for x in sub})|changed|{('migration',x) for x in sub.values()}|adds
if set(cmap)!=expected_keys:die('exact six additions/two substitutions/no removals violated')
# Mandatory identities.
exact={
 ('runtime_pallet','PAL-097'):('package','pallet-orbis-score','upstream_package','indiv-pallet-score','state','present'),
 ('runtime_pallet','PAL-099'):('package','pallet-orbis-honour','upstream_package','indiv-pallet-honour','state','present'),
 ('benchmark','BENCH-41'):('target','pallet_orbis_score','upstream_target','indiv_pallet_score','state','present'),
 ('benchmark','BENCH-43'):('target','pallet_orbis_honour','upstream_target','indiv_pallet_honour','state','present'),
 ('migration','MIG-pallet-orbis-score'):('package','pallet-orbis-score','upstream_package','indiv-pallet-score','state','present-initial-version'),
 ('migration','MIG-pallet-orbis-honour'):('package','pallet-orbis-honour','upstream_package','indiv-pallet-honour','state','present-initial-version')}
for key,items in exact.items():
 r=cmap[key]
 for k,v in zip(items[::2],items[1::2]):
  if r.get(k)!=v:die('identity correction drift '+str(key)+':'+k)
 if key[0]=='migration' and not re.search(r'^storage_version = 1$',r['_body'],re.M):die('migration version drift '+key[1])
# Normalize finite exact union.
norm={'present':[],'planned':[],'excluded':[],'unchecked':[]};seen=set()
for t,i,r in rows:
 ident=t+':'+i
 if ident in seen:die('duplicate '+ident)
 seen.add(ident);state=r.get('state','');status=r.get('status','')
 if t in E:cat=status if status in ('present','planned','pending') else 'unchecked';cat='planned' if cat=='pending' else cat
 elif state.startswith('present'):cat='present'
 elif state.startswith(('planned','pending')):cat='planned'
 elif state.startswith(('excluded','reserved')):cat='excluded'
 elif not state and not status and t in STATELESS:cat=STATELESS[t]
 else:cat='unchecked'
 norm[cat].append(ident)
if norm['unchecked'] or sum(map(len,norm.values()))!=651 or set().union(*map(set,norm.values()))!=seen:die('finite normalized union failure')
expected_counts=(444,139,68) if closed_phase else (443,140,68)
if (len(norm['present']),len(norm['planned']),len(norm['excluded']))!=expected_counts:die('phase counts mismatch')
# Dual source inventory and sole Gate6 phase difference.
inv=(ROOT/'origin/orbis/evidence_inventory_v5.rs').read_text();at=git('show',M+':origin/orbis/evidence_inventory_v5.rs')
if at.returncode or at.stdout!=inv:die('v5 inventory not marker-bound')
def array(name):
 m=re.search(r'pub const '+name+r': &\[.*?= &\[(.*?)\n\];',inv,re.S);return sorted(re.findall(r'\("([^"]*)", "([^"]*)", "([^"]*)", "([^"]*)"\)',m.group(1)))
pending=array('MANIFEST_INVENTORY_V5_PENDING');closed=array('MANIFEST_INVENTORY_V5_CLOSED')
actual=sorted((t,i,r.get('state',''),r.get('status','')) for t,i,r in rows)
if (closed if closed_phase else pending)!=actual or len(pending)!=651:die('phase inventory mismatch')
diffs=[(x,y) for x,y in zip(pending,closed) if x!=y]
if len(diffs)!=1 or diffs[0][0][1:]!=('GATE-6-SLICE2-EVIDENCE','','pending') or diffs[0][1][1:]!=('GATE-6-SLICE2-EVIDENCE','','present'):die('dual inventory differs beyond Gate6')
closed_categories={'present':0,'planned':0,'excluded':0}
for table,_,state,status in closed:
 value=status or state
 if table in E: category='planned' if value in ('pending','planned') else value
 elif state.startswith('present'): category='present'
 elif state.startswith(('planned','pending')): category='planned'
 elif state.startswith(('excluded','reserved')): category='excluded'
 elif not state and not status and table in STATELESS: category=STATELESS[table]
 else: continue
 closed_categories[category]+=1
if closed_categories!={'present':444,'planned':139,'excluded':68}:die('designed closure counts are not 444/139/68')
if a.closure_schema and CLOSURE_DOCS_ONLY_ALLOWLIST!=sorted(CLOSURE_DOCS_ONLY_ALLOWLIST):die('closure docs-only allowlist is not canonical')
for name,arr in [('PENDING',pending),('CLOSED',closed)]:
 digest=hashlib.blake2b(''.join('\0'.join(x)+'\n' for x in arr).encode(),digest_size=32).hexdigest()
 if not re.search(r'MANIFEST_INVENTORY_V5_'+name+r'_BLAKE2_256: &str =\s*"'+digest+'"',inv):die(name+' inventory digest drift')
# Marker registry: five present, Gate6 reserved and absent from emitter calls.
reg=(ROOT/'origin/orbis/evidence_markers_v5.rs').read_text();rat=git('show',M+':origin/orbis/evidence_markers_v5.rs')
if rat.returncode or rat.stdout!=reg:die('v5 marker registry not marker-bound')
registry={i:(g,c,d) for g,i,c,d in re.findall(r'\(\s*"([^"]+)",\s*"([^"]+)",\s*"([^"]+)",\s*"([0-9a-f]{64})"\s*,?\s*\)',reg)}
if set(registry)!={x[1] for x in adds}:die('marker registry must contain five IDs plus reserved Gate6')
if 'emit_evidence_marker_v5("slice2-gate6")' in git('grep','-n','emit_evidence_marker_v5',M,'--','origin/orbis').stdout:die('Gate6 marker emitted')
# Exact phase-specific Gate6 schema.
gate=cmap[('remediation_gate','GATE-6-SLICE2-EVIDENCE')]
if not closed_phase:
 if set(gate)-{'_body','id','order','value','status','planned_slice'} or gate.get('status')!='pending':die('pending Gate6 schema drift')
else:
 required_gate={'_body','id','order','value','status','planned_slice','dependency_ids','architect_review_evidence','critic_review_evidence','artifact_path','artifact_sha256','output_path','output_sha256','source_commit'}
 if set(gate)!=required_gate or gate.get('status')!='present' or gate.get('dependency_ids')!='ARCHITECT-EVIDENCE-CLEAR; CRITIC-EVIDENCE-CLEAR' or gate.get('architect_review_evidence')!=top['evidence_architect_evidence'] or gate.get('critic_review_evidence')!=top['evidence_critic_evidence']:die('closed Gate6 schema drift')
 if gate.get('source_commit')!=M:die('closed Gate6 source binding must remain the pre-registered marker commit')
 for rel in (gate['architect_review_evidence'],gate['critic_review_evidence'],gate['artifact_path'],gate['output_path']):
  path=ROOT/rel
  if not path.is_file() or git('ls-files','--error-unmatch','--',rel).returncode:die('closed tracked review/artifact missing '+rel)
 if sha(ROOT/gate['artifact_path'])!=gate['artifact_sha256'] or sha(ROOT/gate['output_path'])!=gate['output_sha256']:die('closed Gate6 artifact hash drift')
 changed=git('diff','--name-only',pending_commit,'HEAD').stdout.splitlines()
 if changed!=CLOSURE_DOCS_ONLY_ALLOWLIST:die('closure diff is not exact docs-only allowlist')
 if any(path.startswith('origin/orbis/') or path=='Cargo.lock' for path in changed):die('closure changed product/marker/lock')
# A->M test/docs-only allowlist and Cargo.lock invariant.
allowed=[
 'docs/evidence/orbis-v5/S2-BENCHMARK-01.json',
 'docs/evidence/orbis-v5/S2-BENCHMARK-01.out',
 'docs/evidence/orbis-v5/S2-FIXTURES-01.json',
 'docs/evidence/orbis-v5/S2-FIXTURES-01.out',
 'docs/evidence/orbis-v5/S2-MIGRATIONS-01.json',
 'docs/evidence/orbis-v5/S2-MIGRATIONS-01.out',
 'docs/evidence/orbis-v5/S2-PAYOUT-01.json',
 'docs/evidence/orbis-v5/S2-PAYOUT-01.out',
 'docs/evidence/orbis-v5/S2-SURFACES-01.json',
 'docs/evidence/orbis-v5/S2-SURFACES-01.out',
 'docs/evidence/orbis-v5/architect-delta-approval.md',
 'docs/evidence/orbis-v5/architect-product-clear.md',
 'docs/evidence/orbis-v5/critic-delta-approval.md',
 'docs/evidence/orbis-v5/critic-product-clear.md',
 'docs/evidence/orbis-v5/marker-nonruntime-equivalence.json',
 'docs/evidence/orbis-v5/verification-report.json',
 'docs/evidence/orbis-v5/verification-report.sha256',
 'docs/orbis-completion-manifest.toml',
 'origin/orbis/evidence_inventory_v5.rs',
 'origin/orbis/evidence_markers_v5.rs',
 'origin/orbis/pallets/score/src/tests.rs',
 'origin/orbis/runtime/src/meta_v6_fixtures.rs',
 'origin/orbis/runtime/src/tests.rs',
 'scripts/prove-orbis-slice2-v5-nonruntime.sh',
 'scripts/test-verify-orbis-completion-v5.py',
 'scripts/verify-orbis-completion-v5.py',
]
if git('diff','--name-only',A,M).stdout.splitlines()!=allowed:die('A-to-M diff allowlist failure')
if git('show',A+':Cargo.lock').stdout!=git('show',M+':Cargo.lock').stdout:die('Cargo.lock changed A-to-M')
# Cold proof is exact, locked/frozen, poison-controlled.
if not a.static:
 proof_run=subprocess.run([str(ROOT/'scripts/prove-orbis-slice2-v5-nonruntime.sh'),'--check'],cwd=ROOT)
 if proof_run.returncode:die('cold proof --check failed')
proof=json.loads((ROOT/'docs/evidence/orbis-v5/marker-nonruntime-equivalence.json').read_text())
proof_path=ROOT/'docs/evidence/orbis-v5/marker-nonruntime-equivalence.json'
if top.get('nonruntime_equivalence_sha256')!=sha(proof_path):die('cold proof artifact SHA drift')
source_diff=git('diff','--name-only',A,M).stdout.splitlines();source_diff_sha=hashlib.sha256(('\n'.join(source_diff)+'\n').encode()).hexdigest()
if proof.get('source_diff_sha256')!=source_diff_sha:die('cold proof source diff hash drift')
if proof.get('runtime_baseline_commit')!=A or proof.get('evidence_marker_commit')!=M or proof.get('baseline_wasm_sha256')!=proof.get('marker_wasm_sha256') or proof.get('wasm_size_bytes',0)<=0 or proof.get('cargo_lock_equal') is not True or proof.get('poison_control_removed') is not True or proof.get('source_diff')!=allowed or '--locked --frozen' not in proof.get('build_command','') or not proof.get('exclusive_lock_path'):die('cold proof policy failure')
expected_toolchain=subprocess.check_output(['rustc','+1.93.0','--version'],text=True).strip()+'; '+subprocess.check_output(['cargo','+1.93.0','--version'],text=True).strip()
base_lock=git('show',A+':Cargo.lock').stdout.encode();marker_lock=git('show',M+':Cargo.lock').stdout.encode()
proof_exact={
 'toolchain':expected_toolchain,
 'cargo_lock_sha256':hashlib.sha256(base_lock).hexdigest(),
 'marker_cargo_lock_sha256':hashlib.sha256(marker_lock).hexdigest(),
 'features':'default',
 'deterministic_env':'env -u RUNTIME_METADATA_HASH; CARGO_INCREMENTAL=0; SOURCE_DATE_EPOCH=0; TZ=UTC; LC_ALL=C',
 'build_command':'cargo +1.93.0 build --locked --frozen -p origin-orbis-runtime',
 'same_source_path':'/tmp/orbis-v5-slice2-equivalence-fixed',
 'same_target_path':'/tmp/orbis-v5-slice2-equivalence-target',
 'exclusive_lock_path':'/tmp/orbis-v5-slice2-equivalence.lock',
}
if any(proof.get(k)!=v for k,v in proof_exact.items()) or proof.get('cold_target_removed_before_each_build') is not True or proof.get('cmp_exit_code')!=0 or not re.fullmatch(r'[0-9a-f]{64}',proof.get('baseline_wasm_sha256','')):die('cold proof exact reproducibility fields drift')
# Five executable evidence rows/artifacts and exact raw marker ownership.
commands=[]
for ident in ('S2-SURFACES-01','S2-FIXTURES-01','S2-PAYOUT-01','S2-BENCHMARK-01','S2-MIGRATIONS-01'):
 r=cmap[('slice2_evidence',ident)];cmd=r.get('test_or_command','');commands.append(cmd)
 for k in ('source_paths','source_symbol','expected_output','output_sha256','assertion_sha256','artifact_path','artifact_sha256','source_commit'):
  if not r.get(k):die(ident+' missing '+k)
 if r['source_commit']!=M or registry.get(ident,(None,None,None))[2]!=r['assertion_sha256'] or r['expected_assertion']!='assertion='+ident+':'+r['assertion_sha256']:die(ident+' marker binding drift')
 historical=''.join(git('show',M+':'+x.strip()).stdout for x in r['source_paths'].split(';') if not x.strip().startswith('docs/benchmarks/'))
 if r['source_symbol'] not in historical:die(ident+' historical symbol absent')
 group,contract,digest=registry[ident]
 if r.get('value')!=contract or hashlib.sha256(contract.encode()).hexdigest()!=digest:die(ident+' human contract/hash drift')
 source='origin/orbis/pallets/score/src/tests.rs' if ident=='S2-PAYOUT-01' else 'origin/orbis/runtime/src/tests.rs'
 if 'emit_evidence_marker_v5("'+group+'")' not in git('show',M+':'+source).stdout:die(ident+' raw emitter absent')
 ap=ROOT/r['artifact_path'];op=ROOT/f'docs/evidence/orbis-v5/{ident}.out'
 if not ap.is_file() or sha(ap)!=r['artifact_sha256'] or not op.is_file() or sha(op)!=r['output_sha256']:die(ident+' artifact hash drift')
 data=json.loads(ap.read_text())
 expected_fields={k:v for k,v in r.items() if k not in ('_body','artifact_sha256','output_sha256')}
 if data.get('manifest_fields')!=expected_fields or data.get('output_artifact')!=str(op.relative_to(ROOT)) or data.get('output_artifact_sha256')!=r['output_sha256']:die(ident+' complete artifact mirror drift')
 if 'assertion='+ident+':'+r['assertion_sha256'] not in op.read_text():die(ident+' raw marker absent from output')
 if not a.static:
  run=subprocess.run(cmd,cwd=ROOT,shell=True,text=True,stdout=subprocess.PIPE,stderr=subprocess.STDOUT)
  if hashlib.sha256(canonical(cmd,run.stdout,run.returncode).encode()).hexdigest()!=r['output_sha256']:die(ident+' executable output drift')
# Benchmark raw evidence and mechanical inequality are bound independently of the aggregate test.
benchmark=cmap[('slice2_evidence','S2-BENCHMARK-01')]
benchmark_files={
 'benchmark_main_json_sha256':ROOT/'docs/benchmarks/orbis-score-2026-07-13.json',
 'benchmark_main_log_sha256':ROOT/'docs/benchmarks/orbis-score-2026-07-13.log',
 'benchmark_markdown_sha256':ROOT/'docs/benchmarks/orbis-score-2026-07-13.md',
 'benchmark_watch_json_sha256':ROOT/'docs/benchmarks/orbis-score-watch-2026-07-13.json',
 'benchmark_watch_log_sha256':ROOT/'docs/benchmarks/orbis-score-watch-2026-07-13.log',
}
for field,path in benchmark_files.items():
 if not path.is_file() or benchmark.get(field)!=sha(path):die('benchmark supplemental hash drift '+field)
watch=json.loads(benchmark_files['benchmark_watch_json_sha256'].read_text());log=benchmark_files['benchmark_watch_log_sha256'].read_text();md=benchmark_files['benchmark_markdown_sha256'].read_text();ws=(ROOT/'origin/orbis/pallets/score/src/weights.rs').read_text()
set_rows=[row for row in watch if row.get('benchmark')=='set_payout_account']
if len(set_rows)!=1 or min(x['extrinsic_time'] for x in set_rows[0]['time_results'])>25_000:die('set_payout_account JSON measurement/model drift')
if not re.search(r'Extrinsic: "set_payout_account".*?Model:\s*Time ~=\s*25\s*µs.*?Reads = 7\s*Writes = 1\s*Recorded proof Size = 379',log,re.S):die('set_payout_account log model drift')
if benchmark.get('benchmark_compiled_wasm_sha256')!='7bb2b93783280dc512373b124ed1c9ed66eb3dbf434a1be9b4e9b2234df333ac' or '`7bb2b93783280dc512373b124ed1c9ed66eb3dbf434a1be9b4e9b2234df333ac`' not in md:die('recorded benchmark Wasm identity drift')
numeric={'benchmark_measured_ref_time':'25000000','benchmark_measured_proof':'3676','benchmark_measured_reads':'7','benchmark_measured_writes':'1','benchmark_margin':'2','benchmark_configured_writes':'3'}
if any(benchmark.get(k)!=v for k,v in numeric.items()):die('benchmark numeric contract drift')
if 'const SET_PAYOUT_MEASURED_REF_TIME: u64 = 25_000_000;' not in ws or 'const SET_PAYOUT_MARGIN: u64 = 2;' not in ws or not re.search(r'Weight::from_parts\(SET_PAYOUT_MEASURED_REF_TIME\.saturating_mul\(SET_PAYOUT_MARGIN\), 3_676\).*?reads\(7_u64\).*?writes\(3_u64\)',ws,re.S):die('configured payout weight does not dominate measured*2/proof/writes')
# v4 artifacts remain byte-identical by Git history.
if git('diff','--quiet',A,'HEAD','--','docs/evidence/orbis-v4').returncode:die('v4 artifacts changed')
report={'schema':'orbis-completion-verification-v5','phase':('closed' if closed_phase else 'pending-review'),'manifest_sha256':sha(MANIFEST),'audited_runtime_commit':A,'evidence_marker_commit':M,'row_count':651,'present':expected_counts[0],'planned':expected_counts[1],'excluded':68,'pending_inventory_blake2_256':hashlib.blake2b(''.join('\0'.join(x)+'\n' for x in pending).encode(),digest_size=32).hexdigest(),'closed_inventory_blake2_256':hashlib.blake2b(''.join('\0'.join(x)+'\n' for x in closed).encode(),digest_size=32).hexdigest(),'slice2_evidence_present':5,'evidence_present':(121 if closed_phase else 120),'evidence_planned':(4 if closed_phase else 5),'designed_closed_evidence_present':121,'designed_closed_evidence_planned':4,'gate6':('present' if closed_phase else 'pending'),'transition_architect_status':'clear','transition_critic_status':'clear','evidence_architect_status':('clear' if closed_phase else 'pending'),'evidence_critic_status':('clear' if closed_phase else 'pending'),'commands':5,'cold_wasm_sha256':proof['baseline_wasm_sha256'],'cold_wasm_size_bytes':proof['wasm_size_bytes']}
if not a.manifest:
 out=ROOT/'docs/evidence/orbis-v5/verification-report.json';side=ROOT/'docs/evidence/orbis-v5/verification-report.sha256';payload=json.dumps(report,sort_keys=True,indent=2)+'\n';sp=hashlib.sha256(payload.encode()).hexdigest()+'  verification-report.json\n'
 if a.write_report:out.write_text(payload);side.write_text(sp)
 elif not out.is_file() or out.read_text()!=payload or not side.is_file() or side.read_text()!=sp:die('committed report/sidecar drift')
 if not a.write_report:
  required={MANIFEST,proof_path,out,side,ROOT/'scripts/verify-orbis-completion-v5.py',ROOT/'scripts/prove-orbis-slice2-v5-nonruntime.sh'}
  for key in ('transition_architect_evidence','transition_critic_evidence','product_architect_evidence','product_critic_evidence','evidence_architect_evidence','evidence_critic_evidence'):
   if top.get(key):required.add(ROOT/top[key])
  for _,_,row in rows:
   for key in ('artifact_path','output_path'):
    if row.get(key):required.add(ROOT/row[key])
   for rel in row.get('source_paths','').split(';'):
    if rel.strip():required.add(ROOT/rel.strip())
  for path in required:
   try:rel=str(path.resolve().relative_to(ROOT.resolve()))
   except ValueError:die('required path escapes repository '+str(path))
   if not path.is_file() or git('ls-files','--error-unmatch','--',rel).returncode or git('diff','--quiet','HEAD','--',rel).returncode:die('required path is missing, untracked, or dirty '+rel)
  dirty=git('status','--porcelain','--untracked-files=all').stdout.splitlines();dirty=[x for x in dirty if not x.endswith(' .omx/') and ' .omx/' not in x]
  if dirty:die('tracked/untracked production evidence tree is dirty: '+','.join(dirty))
print(json.dumps(report,sort_keys=True))
