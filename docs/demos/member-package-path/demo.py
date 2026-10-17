import subprocess,pathlib,tempfile,os,json,re,sys
INTENT = 'intended_use = "verify package delivery"\n[[crew_members]]\nname = "Ada"\ntheme = "navigator"\ndomain = "packages"\nrole = "checks delivery"\n[[anticipated_questions]]\nquestion = "which package was delivered?"\nanswer_shape = "one package"\nseed_intent = "fixture package"\nsparql = "SELECT ?s WHERE { ?s ?p ?o }"\nexpected = "fixture-result"\n'
def fake(path, body):
 path.write_text("#!/bin/sh\nset -eu\n"+body);path.chmod(0o700)
ns={"INTENT":INTENT,"fake":fake}
for stale,version in [(True,'0.4.2'),(False,'0.4.2'),(True,'0.4.1')]:
 with tempfile.TemporaryDirectory(prefix='seeds-package-probe-') as d:
  r=pathlib.Path(d);b=r/'bin';b.mkdir();c=r/'cargo/bin';c.mkdir(parents=True);(r/'caboodle-intent.toml').write_text(ns['INTENT'])
  body="if [ \"$1\" = --help ]; then echo 'seeds is the beads-compatible tracker'; else echo 'sd VERSION (seeds)'; fi\n"
  ns['fake'](b/'sd',body.replace('VERSION',version));ns['fake'](c/'sd',body.replace('VERSION','0.2.0'))
  if not stale:(c/'sd').unlink()
  ns['fake'](b/'brew',"case \"$*\" in 'update --quiet') exit 0;; 'info --json=v2 scbrown/tap/seeds') echo '{\"formulae\":[{\"full_name\":\"scbrown/tap/seeds\",\"tap\":\"scbrown/tap\",\"pinned\":false,\"versions\":{\"stable\":\"0.4.2\"},\"installed\":[{\"version\":\"0.4.2\"}]}]}'; exit 0;; esac; exit 2\n")
  (r/'camayoc').mkdir();(r/'camayoc/REVISION').write_text('f443bf19974d06ab662c8ae3deeccfbb1e082f48')
  for name,v in [('quipu','0.14.1'),('quipu-server','0.14.1'),('bobbin','0.29.0'),('yupana','0.14.0'),('dp','0.8.0')]:ns['fake'](b/name,"echo '"+name+" "+v+"'\n")
  brew=(b/'brew').read_text();brew=brew.replace(" esac; exit 2", " 'info --json=v2 scbrown/tap/'*) name=${3##*/}; case $name in quipu) v=0.14.1;; bobbin) v=0.29.0;; yupana) v=0.14.0;; desire-path) v=0.8.0;; *) exit 2;; esac; printf '{\"formulae\":[{\"full_name\":\"scbrown/tap/%s\",\"tap\":\"scbrown/tap\",\"pinned\":false,\"versions\":{\"stable\":\"%s\"},\"installed\":[{\"version\":\"%s\"}]}]}' \"$name\" \"$v\" \"$v\"; exit 0;; esac; exit 2");(b/'brew').write_text(brew)
  e={'CABOODLE_CAMAYOC_ROOT':str(r/'camayoc'),'HOME':d,'PATH':str(b)+':/usr/bin:/bin','CARGO_HOME':str(r/'cargo'),'CABOODLE_BREW':str(b/'brew')}
  x=subprocess.run([sys.argv[1],'plan','--profile','everything'],cwd=d,env=e,capture_output=True,text=True);assert x.returncode==0,x.stderr
  p=r/'caboodle-plan.toml';p.write_text(re.sub(r'tools = \[.*?\]', 'tools = ["quipu", "camayoc", "bobbin", "yupana", "desire-path", "seeds"]',p.read_text(),count=1,flags=re.S))
  x=subprocess.run([sys.argv[1],'update'],cwd=d,env=e,capture_output=True,text=True);print(json.dumps({'stale_cargo':stale,'path_version':version,'exit':x.returncode,'stdout':x.stdout,'stderr':x.stderr}),flush=True)

  expected = (not stale or sys.argv[2] == "after") and version == "0.4.2"
  assert (x.returncode == 0) == expected, x.stderr
