#!/usr/bin/env python3
"""Verify normalized, historical and executable Orbis completion evidence v4."""
from pathlib import Path
import argparse,hashlib,json,re,subprocess,sys
P=argparse.ArgumentParser(); P.add_argument('--manifest'); P.add_argument('--static',action='store_true'); a=P.parse_args()
ROOT=Path(__file__).resolve().parents[1]; MANIFEST=Path(a.manifest) if a.manifest else ROOT/'docs/orbis-completion-manifest.toml'; ZERO='0'*64
E={'meta_contract','meta_router_variant','meta_vector','meta_ingress','bulletin_v7_rehearsal','bulletin_v7_contract','provider_v8_contract','remediation_gate'}
def die(x): raise SystemExit('orbis-v4: '+x)
def sha(p): return hashlib.sha256(p.read_bytes()).hexdigest()
def fs(body): return dict(re.findall(r'^([A-Za-z0-9_]+) = "([^"]*)"$',body,re.M))
def git(*x): return subprocess.run(['git',*x],cwd=ROOT,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True,errors='ignore')
def canon(cmd,expected): return hashlib.sha256(f'exit=0\ncommand={cmd}\nexpected_output={expected}\n'.encode()).hexdigest()
text=MANIFEST.read_text(); top=fs(text.split('[[',1)[0]); audit=top.get('audited_runtime_commit','')
if top.get('frozen_at_commit') is None or not re.fullmatch('[0-9a-f]{40}',audit): die('audited_runtime_commit must be full 40-hex')
if git('cat-file','-e',audit+'^{commit}').returncode: die('audited runtime commit does not exist')
if 'implemented-pending-evidence' in text: die('implemented-pending-evidence forbidden')
rows=[]; normalized={'present':[],'planned':[],'excluded':[],'unchecked':[]}; evidence=[]
for table,body in re.findall(r'^\[\[([^]]+)\]\]\n(.*?)(?=^\[\[|\Z)',text,re.M|re.S):
 r=fs(body); ident=r.get('id',r.get('name',r.get('package',table))); state=r.get('state',''); status=r.get('status','')
 if table in E: category=status if status in ('present','planned') else 'unchecked'; evidence.append((table,r))
 elif state.startswith('present'): category='present'
 elif state.startswith(('planned','pending')): category='planned'
 elif state.startswith(('excluded','reserved')): category='excluded'
 elif state: category='unchecked'
 else: category='excluded' # source/provenance inventory is informational, not implementation evidence
 if state.startswith('present') and status=='planned': die(f'{ident} state/status contradiction')
 if state.startswith(('planned','pending')) and status=='present': die(f'{ident} state/status contradiction')
 normalized[category].append(f'{table}:{ident}')
if normalized['unchecked']: die('unchecked rows: '+','.join(normalized['unchecked']))
commands={}
for table,r in evidence:
 ident=r.get('id','?'); status=r.get('status')
 if status=='planned':
  for k in ('source_paths','source_symbol','test_or_command','expected_assertion','artifact_path','artifact_sha256','source_commit','expected_output','output_sha256'):
   if r.get(k,''): die(f'{ident} planned row has nonblank {k}')
  if not r.get('planned_slice') or not r.get('dependency_ids'): die(f'{ident} incomplete planned schema')
  continue
 if status!='present': die(f'{ident} invalid evidence status')
 for k in ('source_paths','source_symbol','test_or_command','expected_assertion','artifact_path','artifact_sha256','source_commit','expected_output','output_sha256'):
  if not r.get(k): die(f'{ident} missing {k}')
 commit=r['source_commit']
 if not re.fullmatch('[0-9a-f]{40}',commit) or git('merge-base','--is-ancestor',commit,audit).returncode: die(f'{ident} source commit is not an audited ancestor')
 historical=''
 for rel in r['source_paths'].split(';'):
  rel=rel.strip(); shown=git('show',commit+':'+rel)
  if shown.returncode: die(f'{ident} historical path missing at {commit}: {rel}')
  historical+=shown.stdout
 if r['source_symbol'] not in historical: die(f'{ident} historical symbol absent: {r["source_symbol"]}')
 cmd=r['test_or_command']; expected=r['expected_output']
 if 'CARGO_TARGET_DIR=target/evidence-v4' not in cmd or 'cargo test ' not in cmd: die(f'{ident} command lacks declared isolated environment')
 if re.search(r'--lib (remediation|completion_manifest)($| )',cmd): die(f'{ident} broad non-mapping command')
 if canon(cmd,expected)!=r['output_sha256']: die(f'{ident} immutable output hash mismatch')
 artifact=ROOT/r['artifact_path']
 if r['artifact_sha256']==ZERO or not artifact.is_file() or sha(artifact)!=r['artifact_sha256']: die(f'{ident} artifact missing/zero/hash mismatch')
 try: data=json.loads(artifact.read_text())
 except Exception: die(f'{ident} artifact is not normalized JSON')
 for k,want in [('id',ident),('source_commit',commit),('source_paths',r['source_paths']),('source_symbol',r['source_symbol']),('command',cmd),('expected',r['expected_assertion']),('expected_output',expected),('output_sha256',r['output_sha256'])]:
  if data.get(k)!=want: die(f'{ident} artifact binding mismatch: {k}')
 commands.setdefault(cmd,expected)
# Explicit lifecycle seams.
if not all(r.get('status')=='present' for t,r in evidence if t in ('bulletin_v7_rehearsal','bulletin_v7_contract')): die('Bulletin V7 incomplete')
if not all(r.get('status')=='planned' for t,r in evidence if t=='provider_v8_contract'): die('provider V8 not purely planned')
ids={r.get('id') for _,r in evidence}; modes={'META-EVIDENCE-COMPILED-ENABLED','META-EVIDENCE-CUSTOM-LOSS','META-EVIDENCE-NOHASH-CANNOTLOOKUP','META-EVIDENCE-EARLY-PROPAGATION'}
if len(ids&modes)!=4: die('metadata modes must be exactly four')
if not a.static:
 for cmd,expected in sorted(commands.items()):
  run=subprocess.run(cmd,cwd=ROOT,shell=True,text=True,stdout=subprocess.PIPE,stderr=subprocess.STDOUT)
  if run.returncode or expected not in run.stdout: die('evidence command failed: '+cmd+'\n'+run.stdout[-2000:])
report={'schema':'orbis-completion-verification-v4','manifest_sha256':sha(MANIFEST),'audited_runtime_commit':audit,'present':len(normalized['present']),'planned':len(normalized['planned']),'excluded':len(normalized['excluded']),'unchecked':normalized['unchecked'],'evidence_present':sum(r.get('status')=='present' for _,r in evidence),'evidence_planned':sum(r.get('status')=='planned' for _,r in evidence),'commands':len(commands),'metadata_evidence':sorted(modes),'bulletin_v7':'present','provider_v8':'planned','critic_status':top.get('critic_status')}
if not a.manifest:
 out=ROOT/'docs/evidence/orbis-v4/verification-report.json'; out.write_text(json.dumps(report,sort_keys=True,indent=2)+'\n'); (out.parent/'verification-report.sha256').write_text(sha(out)+'  verification-report.json\n')
print(json.dumps(report,sort_keys=True))
