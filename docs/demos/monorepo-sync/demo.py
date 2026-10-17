# arming: agent-invoked — private native share contract probe only
import json,pathlib,subprocess,tempfile,os,tarfile,shutil
root=pathlib.Path(tempfile.mkdtemp(prefix="monorepo-contract-"));db=root/"producer.db";binary=os.environ["CANDIDATE_BINARY"]
qp=shutil.which("quipu");assert qp
def run(*args,ok=True):
 p=subprocess.run([str(x) for x in args],capture_output=True,text=True,timeout=60)
 if ok and p.returncode:raise AssertionError((args,p.returncode,p.stdout,p.stderr))
 return p
shape=root/"fixture.ttl";shape.write_text('@prefix sh: <http://www.w3.org/ns/shacl#> . <urn:test:FileShape> a sh:NodeShape ; sh:targetClass <urn:test:File> ; sh:property [ sh:path <urn:test:path> ; sh:minCount 1 ] .')
run(qp,"shapes","load","fixture",shape,"--db",db)
base=pathlib.Path(__file__).resolve().parents[3]/"packs/policy"
run(qp,"shapes","load","identifier-policy",base/"identifier-policy.shapes.ttl","--db",db)
run(qp,"knot",base/"identifier-policy.ttl","--graph","urn:test:policy","--db",db)
print(run(qp,"--version").stdout.strip(),flush=True)
print(run(binary,"--version").stdout.strip(),flush=True)
repo=pathlib.Path(os.environ["RECKONING_FIXTURE"])
assert run("git","-C",repo,"rev-parse","HEAD").stdout.strip()=="e69450e4a449df74cd6b989afa4704b17fc8b977"
paths=subprocess.check_output(["git","-C",str(repo),"ls-tree","-r","--name-only","HEAD","packages"],text=True).splitlines()
selected=[next(p for p in paths if p.startswith("packages/"+name+"/") and p.endswith("package.json")) for name in ["client","server","shared"]]
producer_checkout=root/"producer-checkout";producer_checkout.mkdir()
run("git","init","-q",producer_checkout)
for path in selected:
 target=producer_checkout/path;target.parent.mkdir(parents=True,exist_ok=True);target.write_bytes((repo/path).read_bytes())
run("git","-C",producer_checkout,"add",".")
(producer_checkout/".github").mkdir();(producer_checkout/".github/CODEOWNERS").write_text("* @fallback\n/packages/client/ @clientowner\n/packages/server/ @org/server\n/packages/shared/ @sharedowner\n")
print("CODEOWNERS is an explicit synthetic fixture; pinned Reckoning has none",flush=True)
ttl=root/"files.ttl";ttl.write_text("\n".join('<urn:reckoning:'+name+'> a <urn:test:File> ; <urn:test:path> '+json.dumps(path)+' ; <http://www.w3.org/2000/01/rdf-schema#label> '+json.dumps(name)+' .' for name,path in zip(["client","server","shared"],selected))+'\n<urn:reckoning:client> <urn:test:uses> <urn:reckoning:shared> .')
run(qp,"knot",ttl,"--graph","urn:test:files","--db",db)
baseline=os.environ.get("BASELINE_BINARY")
if baseline:
 p=run(baseline,"packs","project-scope","--graph","urn:test:files","--path-predicate","urn:test:path","--prefix","packages/client/",ok=False)
 assert p.returncode!=0 and "unrecognized subcommand" in p.stderr
 print("BASELINE same project-scope command: refused; command absent",flush=True)
pre=run(binary,"packs","project-scope","--repo",producer_checkout,"--db",db,"--ownership-predicate","urn:test:owned_by","--graph","urn:test:files","--path-predicate","urn:test:path","--prefix","packages/client/",ok=False)
assert pre.returncode!=0 and "absent or unreadable" in pre.stderr
print("unknown owner resource refuses before scope emission",flush=True)
owner_nodes=root/"owners.ttl";owner_nodes.write_text('\n'.join('<'+iri+'> <http://www.w3.org/2000/01/rdf-schema#label> "fixture owner" .' for iri in ["https://github.com/clientowner","https://github.com/orgs/org/teams/server","https://github.com/sharedowner"]))
run(qp,"knot",owner_nodes,"--graph","urn:test:files","--db",db)
parent=root/"parent"
run(qp,"share","--graph","urn:test:files","--output",parent,"--shapes","fixture","--no-queries","--db",db)
pid=json.loads((parent/"manifest.json").read_text())["share_id"]
for name in ["client","server","shared"]:
 prefix="packages/"+name+"/"
 query=run(binary,"packs","project-scope","--repo",producer_checkout,"--db",db,"--ownership-predicate","urn:test:owned_by","--graph","urn:test:files","--path-predicate","urn:test:path","--prefix",prefix).stdout.strip()
 out=root/name
 run(qp,"share","--construct",query,"--parent-share",pid,"--output",out,"--shapes","fixture","--no-queries","--db",db)
 print("native scope:",prefix,flush=True)
 manifest=json.loads((out/"manifest.json").read_text())
 assert manifest["parent_share"]==pid and manifest["scope"]=={"kind":"construct","value":query}
 data=(out/"export.nt").read_text();assert '<urn:reckoning:'+name+'>' in data
 if name=="client":assert '<urn:test:uses> <urn:reckoning:shared>' in data
 print(name,"native parent_share/scope preserved; export lines",len(data.splitlines()),flush=True)
print("Three native derived shares preserve parent/scope and ownership; client-to-shared edge retained.")

consumer=root/"consumer.db";checkout=root/"checkout";checkout.mkdir()
run("git","init","-q",checkout);run("git","-C",checkout,"remote","add","origin","https://example.org/owner/monorepo.git")
packs=checkout/".quipu/packs";packs.mkdir(parents=True)
for name in ["parent","client","server","shared"]:
 with tarfile.open(packs/(name+".pendant.tar.gz"),"w:gz") as archive:archive.add(root/name,arcname="pendant")
for name in ["client","server","shared"]:
 target=checkout/"packages"/name;target.mkdir(parents=True);(target/"package.json").write_bytes((repo/"packages"/name/"package.json").read_bytes())
(checkout/"pnpm-workspace.yaml").write_bytes((repo/"pnpm-workspace.yaml").read_bytes())
run("git","-C",checkout,"add",".")
run("git","-C",checkout,"-c","user.name=Fixture","-c","user.email=fixture@example.invalid","commit","-q","-m","private monorepo fixture")
authority=root/"authority"
run(qp,"share","--construct","CONSTRUCT { ?s ?p ?o } WHERE { ?s ?p ?o FILTER(?s = <urn:test:absent>) }","--output",authority,"--shapes","fixture","--no-queries","--db",db)
auth=root/"authority.pendant.tar.gz"
with tarfile.open(auth,"w:gz") as archive:archive.add(authority,arcname="pendant")
trust=root/"trust.toml";trust.write_text("owners=['example.org/owner']\n")
binary=os.environ["CANDIDATE_BINARY"]
def sync(ok=True):return run(binary,"packs","sync","--monorepo","--repo",checkout,"--shapes-from",auth,"--trust-file",trust,"--db",consumer,ok=ok)
if baseline:
 flat=root/"flat-control.db"
 flat_args=["packs","sync","--repo",checkout,"--shapes-from",auth,"--trust-file",trust,"--db",flat]
 assert json.loads(run(binary,*flat_args).stdout)["packs"]==4
 assert json.loads(run(baseline,*flat_args).stdout)["outcome"]=="unchanged"
 print("flat mode retains v1 receipts readable by the baseline binary",flush=True)
report=json.loads(sync().stdout);print("initial:",report["outcome"],"projects:",report["packs"],flush=True);assert report["packs"]==3
assert json.loads(sync().stdout)["outcome"]=="unchanged"
def ask(body):return run(qp,"read","ASK FROM <urn:quipu:repo:example.org/owner/monorepo:current> { "+body+" }","--db",consumer).stdout.strip()
assert ask("<urn:reckoning:client> <urn:test:uses> <urn:reckoning:shared>")=="true"
assert ask("<urn:reckoning:client> <urn:test:owned_by> <https://github.com/clientowner>")=="true"
old_members={m["key"]:m["graph"] for m in report["current"]["members"]}
extra=root/"client-change.ttl";extra.write_text('<urn:reckoning:client> <http://www.w3.org/2000/01/rdf-schema#label> "client-updated" .')
run(qp,"knot",extra,"--graph","urn:test:files","--db",db)
client_query=json.loads((root/"client/manifest.json").read_text())["scope"]["value"]
changed=root/"client-updated"
run(qp,"share","--construct",client_query,"--parent-share",pid,"--output",changed,"--shapes","fixture","--no-queries","--db",db)
with tarfile.open(packs/"client.pendant.tar.gz","w:gz") as archive:archive.add(changed,arcname="pendant")
report=json.loads(sync().stdout);new_members={m["key"]:m["graph"] for m in report["current"]["members"]}
assert new_members["client.pendant.tar.gz"]!=old_members["client.pendant.tar.gz"]
for key in ["server.pendant.tar.gz","shared.pendant.tar.gz"]:assert new_members[key]==old_members[key]
assert ask('<urn:reckoning:client> <http://www.w3.org/2000/01/rdf-schema#label> "client-updated"')=="true"
assert ask("<urn:reckoning:client> <urn:test:uses> <urn:reckoning:shared>")=="true"
assert ask("<urn:reckoning:client> <urn:test:owned_by> <https://github.com/clientowner>")=="true"
print("changed client gets a new pack graph; server/shared graph identities reused; ownership/cross-edge survive",flush=True)
run(binary,"init","--pack-hooks","--monorepo","--repo",checkout,"--shapes-from",auth,"--trust-file",trust,"--db",consumer)
hook_event=run("git","-C",checkout,"checkout","--detach","HEAD")
hook_output=hook_event.stdout+hook_event.stderr
start=hook_output.find("{");assert start>=0
hook_report=json.JSONDecoder().raw_decode(hook_output[start:])[0]
assert hook_report["packs"]==3
print("actual Git checkout event preserves opt-in monorepo mode: three projects, parent excluded",flush=True)
run("git","-C",checkout,"sparse-checkout","init","--cone")
run("git","-C",checkout,"sparse-checkout","set",".quipu","packages/client","packages/shared")
assert not (checkout/"packages/server").exists()
report=json.loads(sync().stdout);print("sparse:",report["outcome"],"projects:",report["packs"],flush=True);assert report["packs"]==2
assert ask("<urn:reckoning:server> ?p ?o")=="false"
assert ask("<urn:reckoning:client> <urn:test:uses> <urn:reckoning:shared>")=="true"
assert run(qp,"read","ASK { <urn:reckoning:client> ?p ?o }","--db",consumer).stdout.strip()=="false"
print("sparse server absent, cross-project edge survives, ROOT unchanged",flush=True)
receipt=pathlib.Path(str(consumer)+".caboodle-packs.json");before=receipt.read_bytes()
with (root/"server/export.nt").open("a") as f:f.write('<urn:tampered> <urn:bad> "bad" .\n')
with tarfile.open(packs/"server.pendant.tar.gz","w:gz") as archive:archive.add(root/"server",arcname="pendant")
p=sync(False);assert p.returncode!=0 and receipt.read_bytes()==before
assert ask("<urn:reckoning:client> <urn:test:uses> <urn:reckoning:shared>")=="true"
print("tampered absent project refuses, receipt and CURRENT retained",flush=True)
