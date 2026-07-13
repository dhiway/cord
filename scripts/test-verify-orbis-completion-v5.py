#!/usr/bin/env python3
"""Adversarial mutation tests for the Orbis v5 pending verifier."""
from pathlib import Path
import json,os,subprocess,sys,tempfile
ROOT=Path(__file__).resolve().parents[1]; verifier=ROOT/'scripts/verify-orbis-completion-v5.py'; source=(ROOT/'docs/orbis-completion-manifest.toml').read_text()
env=os.environ.copy();env['ORBIS_V5_SKIP_V4']='1'
def rejected(label,text):
 with tempfile.NamedTemporaryFile('w',suffix='.toml',delete=False) as f:f.write(text);name=f.name
 try:r=subprocess.run([sys.executable,str(verifier),'--static','--manifest',name],cwd=ROOT,env=env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,text=True)
 finally:Path(name).unlink(missing_ok=True)
 if r.returncode==0:raise SystemExit('mutation accepted: '+label)
 print('rejected='+label)
def one(old,new):
 if source.count(old)!=1:raise SystemExit('mutation anchor not unique: '+old)
 return source.replace(old,new,1)
rejected('unrelated-row-body-status',one('id = "PAL-000"\nname = "system"\nindex = 0\npackage = "frame-system"','id = "PAL-000"\nname = "system-mutated"\nindex = 0\npackage = "frame-system"'))
rejected('local-upstream-package-identity',source.replace('package = "pallet-orbis-score"\nupstream_package = "indiv-pallet-score"','package = "indiv-pallet-score"\nupstream_package = "pallet-orbis-score"'))
rejected('migration-substitution',one('id = "MIG-pallet-orbis-score"','id = "MIG-indiv-pallet-score"'))
rejected('finite-count-addition',source+'\n[[slice2_evidence]]\nid = "S2-UNAPPROVED"\nstatus = "present"\n')
rejected('staged-gate6',one('id = "GATE-6-SLICE2-EVIDENCE"\norder = 6\nvalue = "independent evidence review of the bounded Slice 2 v5 transition"\nstatus = "pending"','id = "GATE-6-SLICE2-EVIDENCE"\norder = 6\nvalue = "independent evidence review of the bounded Slice 2 v5 transition"\nstatus = "present"'))
rejected('source-binding',source.replace('source_commit = "755b3681983d90ca79c1eb7083f3a904370d10d3"','source_commit = "317a3a5b3dda0964958e08b94c683b1cc1b8e387"'))
rejected('output-binding',one('output_sha256 = "4cac1113f0c43fda8e34c358862666440cb6ff0cef63de03215f5cfcf527d69c"','output_sha256 = "'+'0'*64+'"'))
proof=ROOT/'docs/evidence/orbis-v5/marker-nonruntime-equivalence.json';original=proof.read_bytes()
try:
 data=json.loads(original);data['poison_control_removed']=False;proof.write_text(json.dumps(data,sort_keys=True,indent=2)+'\n')
 rejected('cold-poison-control',source)
finally:proof.write_bytes(original)
# Clean-tree enforcement is tested only when the caller starts from a clean tracked tree.
status=subprocess.run(['git','status','--porcelain','--untracked-files=all'],cwd=ROOT,text=True,stdout=subprocess.PIPE).stdout.splitlines()
status=[line for line in status if ' .omx/' not in line]
if not status:
 dirty=ROOT/'docs/evidence/orbis-v5/UNTRACKED-POISON';dirty.write_text('poison')
 try:
  r=subprocess.run([sys.executable,str(verifier),'--static'],cwd=ROOT,env=env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,text=True)
  if r.returncode==0:raise SystemExit('dirty/untracked evidence accepted')
  print('rejected=dirty-untracked')
 finally:dirty.unlink()
print('orbis-v5-adversarial=ok')
