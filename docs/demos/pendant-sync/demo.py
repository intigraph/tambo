# arming: agent-invoked — explicit native demo; no installed-store or hook changes
import hashlib,json,os,pathlib,shutil,subprocess,tarfile,tempfile
root=pathlib.Path(tempfile.mkdtemp(prefix='pendant-native-'));source=pathlib.Path(__file__).resolve().parents[3];binary=pathlib.Path(os.environ['CANDIDATE_BINARY']);qp=shutil.which('quipu');assert qp
print('Target: private temporary Git repository and SQLite stores')
def run(*args,ok=True):
 p=subprocess.run([str(a) for a in args],capture_output=True,text=True,timeout=60)
 if ok and p.returncode:raise AssertionError((args,p.returncode,p.stdout,p.stderr))
 return p
repo=root/'repo';repo.mkdir();run('git','init','-q',repo);run('git','-C',repo,'remote','add','origin','https://example.org/owner/project.git')
packs=repo/'.quipu/packs';packs.mkdir(parents=True)
trust=root/'trust.toml';trust.write_text("owners=['example.org/owner']\n")
shape=root/'fixture.ttl';shape.write_text('''@prefix sh: <http://www.w3.org/ns/shacl#> .
<urn:fixture:shape> a sh:NodeShape ; sh:targetClass <urn:fixture:Thing> ; sh:property [ sh:path <http://www.w3.org/2000/01/rdf-schema#label> ; sh:minCount 1 ] .
''')
db=root/'producer.db'
run(qp,'shapes','load','fixture',shape,'--db',db)
run(qp,'shapes','load','identifier-policy',source/'packs/policy/identifier-policy.shapes.ttl','--db',db)
run(qp,'knot',source/'packs/policy/identifier-policy.ttl','--graph','urn:fixture:policy','--db',db)
def bundle(name,label,graph):
 if label is not None:
  ttl=root/(name+'.ttl');ttl.write_text('<urn:fixture:one> a <urn:fixture:Thing> ; <http://www.w3.org/2000/01/rdf-schema#label> '+json.dumps(label)+' .')
  run(qp,'knot',ttl,'--graph',graph,'--db',db)
 out=root/name
 scope=['--graph',graph] if label is not None else ['--construct','CONSTRUCT { ?s ?p ?o } WHERE { ?s ?p ?o FILTER(?s = <urn:fixture:absent>) }']
 run(qp,'share',*scope,'--output',out,'--shapes','fixture','--no-queries','--db',db)
 archive=root/(name+'.pendant.tar.gz')
 with tarfile.open(archive,'w:gz') as t:t.add(out,arcname='pendant')
 return archive,out
authority,_=bundle('authority',None,'urn:fixture:empty')
archive,share=bundle('one','one','urn:fixture:one-graph');shutil.copyfile(archive,packs/'one.pendant.tar.gz')
consumer=root/'consumer.db'
def sync(ok=True):return run(binary,'packs','sync','--repo',repo,'--shapes-from',authority,'--trust-file',trust,'--db',consumer,ok=ok)
baseline=os.environ.get('BASELINE_BINARY')
if baseline:
 p=run(baseline,'packs','sync','--repo',repo,'--shapes-from',authority,'--trust-file',trust,'--db',consumer,ok=False)
 assert p.returncode!=0 and 'unrecognized subcommand' in p.stderr and not consumer.exists()
 print('BASELINE same sync command: refused (sync subcommand absent); consumer database absent')
p=sync();print('CANDIDATE initial:',json.loads(p.stdout)['outcome']);assert json.loads(p.stdout)['outcome']=='composed'
p=sync();print('repeat:',json.loads(p.stdout)['outcome']);assert json.loads(p.stdout)['outcome']=='unchanged'
archive2,share2=bundle('two','two','urn:fixture:two-graph');shutil.copyfile(archive2,packs/'one.pendant.tar.gz')
p=sync();assert json.loads(p.stdout)['outcome']=='composed';print('changed:',json.loads(p.stdout)['outcome'])
def ask(value,dataset='current'):
 q='ASK FROM <urn:quipu:repo:example.org/owner/project:'+dataset+'> { <urn:fixture:one> <http://www.w3.org/2000/01/rdf-schema#label> '+json.dumps(value)+' }'
 return run(qp,'read',q,'--db',consumer).stdout.strip()
assert ask('two')=='true';assert ask('one','previous')=='true';print('CURRENT two / PREVIOUS one: PASS')
receipt=root/'consumer.db.caboodle-packs.json';before=receipt.read_bytes();db_before=hashlib.sha256(consumer.read_bytes()).hexdigest()
with (share2/'export.nt').open('a') as f:f.write('<urn:tampered> <urn:value> "bad" .\n')
with tarfile.open(packs/'one.pendant.tar.gz','w:gz') as t:t.add(share2,arcname='pendant')
p=sync(False);print('tampered:',p.returncode,p.stderr.strip().replace(str(root), '<fixture>'));assert p.returncode!=0;assert receipt.read_bytes()==before;assert hashlib.sha256(consumer.read_bytes()).hexdigest()==db_before;assert ask('two')=='true'
print('Tampered archive refused; receipt/database/CURRENT unchanged: PASS')
(packs/'one.pendant.tar.gz').unlink();p=sync();assert json.loads(p.stdout)['outcome']=='composed';assert ask('two')=='false';print('Removed pack no longer in CURRENT: PASS')
p=run(qp,'read','ASK { <urn:fixture:one> <http://www.w3.org/2000/01/rdf-schema#label> ?o }','--db',consumer);assert p.stdout.strip()=='false';print('ROOT untouched: PASS')
