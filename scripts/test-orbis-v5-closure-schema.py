#!/usr/bin/env python3
"""Build a real temporary closed Git commit and exercise both valid and hostile closure states."""
from pathlib import Path
import hashlib,json,os,re,shutil,subprocess,sys,tempfile
ROOT=Path(__file__).resolve().parents[1]; pending=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip();tmp=Path(tempfile.mkdtemp(prefix='orbis-v5-closure-'))
def run(*args,**kw):return subprocess.run(args,cwd=tmp,text=True,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,**kw)
try:
 subprocess.run(['git','worktree','add','--detach',str(tmp),pending],cwd=ROOT,check=True,stdout=subprocess.DEVNULL)
 p=tmp/'docs/orbis-completion-manifest.toml';s=p.read_text();
 s=s.replace('evidence_phase = "pending-review"','evidence_phase = "closed"').replace('evidence_architect_evidence = ""','evidence_architect_evidence = "docs/evidence/orbis-v5/architect-evidence-review-clear.md"').replace('evidence_architect_status = "pending"','evidence_architect_status = "clear"').replace('evidence_critic_evidence = ""','evidence_critic_evidence = "docs/evidence/orbis-v5/critic-evidence-review-clear.md"').replace('evidence_critic_status = "pending"','evidence_critic_status = "clear"')
 anchor='evidence_phase = "closed"';s=s.replace(anchor,anchor+'\npending_evidence_commit = "'+pending+'"')
 pat=re.compile(r'\[\[remediation_gate\]\]\nid = "GATE-6-SLICE2-EVIDENCE".*?(?=\n\[\[|\Z)',re.S);old=pat.search(s).group(0)
 contract='Independent Architect and Critic evidence reviews clear the bounded Slice 2 v5 evidence transition.';digest=hashlib.sha256(contract.encode()).hexdigest();out='assertion=GATE-6-SLICE2-EVIDENCE:'+digest+'\n';op=tmp/'docs/evidence/orbis-v5/GATE-6-SLICE2-EVIDENCE.out';op.write_text(out);oh=hashlib.sha256(out.encode()).hexdigest()
 for name,role in [('architect','Architect'),('critic','Critic')]:
  (tmp/f'docs/evidence/orbis-v5/{name}-evidence-review-clear.md').write_text(f'# {role} Slice 2 v5 evidence review — CLEAR\n\nIndependent evidence review of pending commit `{pending}` is CLEAR for the bounded Slice 2 transition only.\n')
 gate_fields={'id':'GATE-6-SLICE2-EVIDENCE','order':'6','value':contract,'status':'present','planned_slice':'slice2-evidence-review','dependency_ids':'ARCHITECT-EVIDENCE-CLEAR; CRITIC-EVIDENCE-CLEAR','architect_review_evidence':'docs/evidence/orbis-v5/architect-evidence-review-clear.md','critic_review_evidence':'docs/evidence/orbis-v5/critic-evidence-review-clear.md','artifact_path':'docs/evidence/orbis-v5/GATE-6-SLICE2-EVIDENCE.json','output_path':'docs/evidence/orbis-v5/GATE-6-SLICE2-EVIDENCE.out','output_sha256':oh,'source_commit':'ce60319f85a13b29e143a4d15189616f38abe777'}
 ap=tmp/'docs/evidence/orbis-v5/GATE-6-SLICE2-EVIDENCE.json';ap.write_text(json.dumps({'schema':'orbis-v5-gate6-closure-v1','fields':gate_fields,'assertion_sha256':digest},sort_keys=True,indent=2)+'\n');gate_fields['artifact_sha256']=hashlib.sha256(ap.read_bytes()).hexdigest()
 order=['id','order','value','status','planned_slice','dependency_ids','architect_review_evidence','critic_review_evidence','artifact_path','artifact_sha256','output_path','output_sha256','source_commit'];body='[[remediation_gate]]\n'+''.join(f'{k} = "{gate_fields[k]}"\n' for k in order);s=s.replace(old,body.rstrip());p.write_text(s)
 # Report files must participate in the exact closure diff before report generation.
 (tmp/'docs/evidence/orbis-v5/verification-report.json').write_text('{}\n');(tmp/'docs/evidence/orbis-v5/verification-report.sha256').write_text('pending\n')
 run('git','config','user.email','closure@example.invalid');run('git','config','user.name','Orbis Closure Test');r=run('git','add','docs');assert r.returncode==0,r.stdout;r=run('git','commit','-m','synthetic closed evidence');assert r.returncode==0,r.stdout
 env=os.environ.copy();env['ORBIS_V5_SYNTHETIC_CLOSURE']='1';env['ORBIS_V5_SKIP_V4']='1'
 r=subprocess.run([sys.executable,str(tmp/'scripts/verify-orbis-completion-v5.py'),'--static','--write-report'],cwd=tmp,env=env,text=True,stdout=subprocess.PIPE,stderr=subprocess.STDOUT);assert r.returncode==0,r.stdout
 run('git','add','docs/evidence/orbis-v5/verification-report.json','docs/evidence/orbis-v5/verification-report.sha256');r=run('git','commit','--amend','--no-edit');assert r.returncode==0,r.stdout
 r=subprocess.run([sys.executable,str(tmp/'scripts/verify-orbis-completion-v5.py'),'--static'],cwd=tmp,env=env,text=True,stdout=subprocess.PIPE,stderr=subprocess.STDOUT);assert r.returncode==0,r.stdout;print('closure-valid=accepted')
 def rejected(label,mutate,restore):
  mutate();x=subprocess.run([sys.executable,str(tmp/'scripts/verify-orbis-completion-v5.py'),'--static'],cwd=tmp,env=env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,text=True);restore();assert x.returncode!=0,label;print('closure-rejected='+label)
 review=tmp/'docs/evidence/orbis-v5/critic-evidence-review-clear.md';raw=review.read_bytes();rejected('one-review',lambda:review.unlink(),lambda:review.write_bytes(raw))
 raw=p.read_bytes();rejected('mixed-phase',lambda:p.write_bytes(raw.replace(b'evidence_critic_status = "clear"',b'evidence_critic_status = "pending"')),lambda:p.write_bytes(raw))
 poison=tmp/'docs/evidence/orbis-v5/untracked-review.md';rejected('untracked-review',lambda:poison.write_text('poison'),lambda:poison.unlink())
 print('orbis-v5-closure-schema=ok')
finally:
 subprocess.run(['git','worktree','remove','--force',str(tmp)],cwd=ROOT,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL);shutil.rmtree(tmp,ignore_errors=True)
