#!/usr/bin/env python3
"""Verify the bounded Orbis Slice 2 manifest-v5 transition in pending or designed closure phase."""
from pathlib import Path
import argparse,hashlib,json,os,re,subprocess,sys,tempfile
P=argparse.ArgumentParser();P.add_argument('--manifest');P.add_argument('--static',action='store_true');P.add_argument('--write-report',action='store_true');P.add_argument('--closure-schema',action='store_true');a=P.parse_args()
ROOT=Path(__file__).resolve().parents[1]; MANIFEST=Path(a.manifest) if a.manifest else ROOT/'docs/orbis-completion-manifest.toml'
A='317a3a5b3dda0964958e08b94c683b1cc1b8e387';M='755b3681983d90ca79c1eb7083f3a904370d10d3';ZERO='0'*64
E={'meta_contract','meta_router_variant','meta_vector','meta_ingress','bulletin_v7_rehearsal','bulletin_v7_contract','provider_v8_contract','remediation_gate','slice2_evidence'}
STATELESS={'source':'excluded','package_provenance':'excluded','protocol_constant':'excluded','protocol_dependency':'excluded'}
CLOSURE_DOCS_ONLY_ALLOWLIST=[
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
for k,v in [('frozen_at_commit',A),('audited_runtime_commit',A),('evidence_marker_commit',M),('evidence_phase','pending-review'),('transition_architect_status','clear'),('transition_critic_status','clear'),('product_architect_status','clear'),('product_critic_status','clear'),('evidence_architect_status','pending'),('evidence_critic_status','pending'),('evidence_architect_evidence',''),('evidence_critic_evidence','')]:
 if top.get(k)!=v:die('pending transition field mismatch '+k)
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
if (len(norm['present']),len(norm['planned']),len(norm['excluded']))!=(443,140,68):die('pending counts are not 443/140/68')
# Dual source inventory and sole Gate6 phase difference.
inv=(ROOT/'origin/orbis/evidence_inventory_v5.rs').read_text();at=git('show',M+':origin/orbis/evidence_inventory_v5.rs')
if at.returncode or at.stdout!=inv:die('v5 inventory not marker-bound')
def array(name):
 m=re.search(r'pub const '+name+r': &\[.*?= &\[(.*?)\n\];',inv,re.S);return sorted(re.findall(r'\("([^"]*)", "([^"]*)", "([^"]*)", "([^"]*)"\)',m.group(1)))
pending=array('MANIFEST_INVENTORY_V5_PENDING');closed=array('MANIFEST_INVENTORY_V5_CLOSED')
actual=sorted((t,i,r.get('state',''),r.get('status','')) for t,i,r in rows)
if pending!=actual or len(pending)!=651:die('pending inventory mismatch')
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
registry={i:(g,d) for g,i,d in re.findall(r'\(\s*"([^"]+)",\s*"([^"]+)",\s*"([0-9a-f]{64})"\s*,?\s*\)',reg)}
if set(registry)!={x[1] for x in adds}:die('marker registry must contain five IDs plus reserved Gate6')
if 'emit_evidence_marker_v5("slice2-gate6")' in git('grep','-n','emit_evidence_marker_v5',M,'--','origin/orbis').stdout:die('Gate6 marker emitted')
# Exact pending gate schema: no implementation/review fields.
gate=cmap[('remediation_gate','GATE-6-SLICE2-EVIDENCE')]
if set(gate)-{'_body','id','order','value','status','planned_slice'} or gate.get('status')!='pending':die('pending Gate6 schema drift')
# A->M test/docs-only allowlist and Cargo.lock invariant.
allowed=['docs/evidence/orbis-v5/architect-delta-approval.md','docs/evidence/orbis-v5/architect-product-clear.md','docs/evidence/orbis-v5/critic-delta-approval.md','docs/evidence/orbis-v5/critic-product-clear.md','origin/orbis/evidence_inventory_v5.rs','origin/orbis/evidence_markers_v5.rs','origin/orbis/pallets/score/src/tests.rs','origin/orbis/runtime/src/meta_v6_fixtures.rs','origin/orbis/runtime/src/tests.rs']
if git('diff','--name-only',A,M).stdout.splitlines()!=allowed:die('A-to-M diff allowlist failure')
if git('show',A+':Cargo.lock').stdout!=git('show',M+':Cargo.lock').stdout:die('Cargo.lock changed A-to-M')
# Cold proof is exact, locked/frozen, poison-controlled.
proof=json.loads((ROOT/'docs/evidence/orbis-v5/marker-nonruntime-equivalence.json').read_text())
if proof.get('runtime_baseline_commit')!=A or proof.get('evidence_marker_commit')!=M or proof.get('baseline_wasm_sha256')!=proof.get('marker_wasm_sha256') or proof.get('wasm_size_bytes',0)<=0 or proof.get('cargo_lock_equal') is not True or proof.get('poison_control_removed') is not True or proof.get('source_diff')!=allowed or '--locked --frozen' not in proof.get('build_command','') or not proof.get('exclusive_lock_path'):die('cold proof policy failure')
# Five executable evidence rows/artifacts and exact raw marker ownership.
commands=[]
for ident in ('S2-SURFACES-01','S2-FIXTURES-01','S2-PAYOUT-01','S2-BENCHMARK-01','S2-MIGRATIONS-01'):
 r=cmap[('slice2_evidence',ident)];cmd=r.get('test_or_command','');commands.append(cmd)
 for k in ('source_paths','source_symbol','expected_output','output_sha256','assertion_sha256','artifact_path','artifact_sha256','source_commit'):
  if not r.get(k):die(ident+' missing '+k)
 if r['source_commit']!=M or registry.get(ident,(None,None))[1]!=r['assertion_sha256'] or r['expected_assertion']!='assertion='+ident+':'+r['assertion_sha256']:die(ident+' marker binding drift')
 historical=''.join(git('show',M+':'+x.strip()).stdout for x in r['source_paths'].split(';') if not x.strip().startswith('docs/benchmarks/'))
 if r['source_symbol'] not in historical:die(ident+' historical symbol absent')
 group=registry[ident][0]
 source='origin/orbis/pallets/score/src/tests.rs' if ident=='S2-PAYOUT-01' else 'origin/orbis/runtime/src/tests.rs'
 if 'emit_evidence_marker_v5("'+group+'")' not in git('show',M+':'+source).stdout:die(ident+' raw emitter absent')
 ap=ROOT/r['artifact_path'];op=ROOT/f'docs/evidence/orbis-v5/{ident}.out'
 if not ap.is_file() or sha(ap)!=r['artifact_sha256'] or not op.is_file() or sha(op)!=r['output_sha256']:die(ident+' artifact hash drift')
 data=json.loads(ap.read_text())
 if data.get('id')!=ident or data.get('output_artifact')!=str(op.relative_to(ROOT)) or data.get('output_sha256')!=r['output_sha256']:die(ident+' artifact binding drift')
 if 'assertion='+ident+':'+r['assertion_sha256'] not in op.read_text():die(ident+' raw marker absent from output')
 if not a.static:
  run=subprocess.run(cmd,cwd=ROOT,shell=True,text=True,stdout=subprocess.PIPE,stderr=subprocess.STDOUT)
  if hashlib.sha256(canonical(cmd,run.stdout,run.returncode).encode()).hexdigest()!=r['output_sha256']:die(ident+' executable output drift')
# Benchmark raw evidence and mechanical inequality.
bj=ROOT/'docs/benchmarks/orbis-score-watch-2026-07-13.json';bl=ROOT/'docs/benchmarks/orbis-score-watch-2026-07-13.log';ws=(ROOT/'origin/orbis/pallets/score/src/weights.rs').read_text()
if not bj.is_file() or not bl.is_file() or 'set_payout_account' not in bj.read_text() or 'set_payout_account' not in bl.read_text() or 'SET_PAYOUT_MEASURED_REF_TIME: u64 = 25_000_000' not in ws or 'Weight::from_parts(SET_PAYOUT_MEASURED_REF_TIME.saturating_mul(SET_PAYOUT_MARGIN), 3_676)' not in ws:die('benchmark JSON/log/measurement binding failure')
# v4 artifacts remain byte-identical by Git history.
if git('diff','--quiet',A,'HEAD','--','docs/evidence/orbis-v4').returncode:die('v4 artifacts changed')
report={'schema':'orbis-completion-verification-v5','phase':'pending-review','manifest_sha256':sha(MANIFEST),'audited_runtime_commit':A,'evidence_marker_commit':M,'row_count':651,'present':443,'planned':140,'excluded':68,'pending_inventory_blake2_256':hashlib.blake2b(''.join('\0'.join(x)+'\n' for x in pending).encode(),digest_size=32).hexdigest(),'closed_inventory_blake2_256':hashlib.blake2b(''.join('\0'.join(x)+'\n' for x in closed).encode(),digest_size=32).hexdigest(),'slice2_evidence_present':5,'gate6':'pending','transition_architect_status':'clear','transition_critic_status':'clear','evidence_architect_status':'pending','evidence_critic_status':'pending','commands':5,'cold_wasm_sha256':proof['baseline_wasm_sha256'],'cold_wasm_size_bytes':proof['wasm_size_bytes']}
if not a.manifest:
 out=ROOT/'docs/evidence/orbis-v5/verification-report.json';side=ROOT/'docs/evidence/orbis-v5/verification-report.sha256';payload=json.dumps(report,sort_keys=True,indent=2)+'\n';sp=hashlib.sha256(payload.encode()).hexdigest()+'  verification-report.json\n'
 if a.write_report:out.write_text(payload);side.write_text(sp)
 elif not out.is_file() or out.read_text()!=payload or not side.is_file() or side.read_text()!=sp:die('committed report/sidecar drift')
 if not a.write_report:
  dirty=git('status','--porcelain','--untracked-files=all').stdout.splitlines();dirty=[x for x in dirty if not x.endswith(' .omx/') and ' .omx/' not in x]
  if dirty:die('tracked/untracked production evidence tree is dirty: '+','.join(dirty))
print(json.dumps(report,sort_keys=True))
