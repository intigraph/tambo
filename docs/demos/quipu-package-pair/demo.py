"""Run both real installer CLIs with a disposable stateful Homebrew fixture."""
import json,os,pathlib,re,subprocess,sys,tempfile
INTENT='''intended_use = "verify package delivery"
[[crew_members]]
name = "Ada"
theme = "navigator"
domain = "packages"
role = "checks delivery"
[[anticipated_questions]]
question = "which package was delivered?"
answer_shape = "one package"
seed_intent = "fixture package"
sparql = "SELECT ?s WHERE { ?s ?p ?o }"
expected = "fixture-result"
'''
BREW='''echo "$*" >> "$HOME/brew.log"
case "$*" in
 "update --quiet") cp "$HOME/remote-stable" "$HOME/local-stable"; exit 0;;
 "info --json=v2 scbrown/tap/bobbin")
  printf '{"formulae":[{"full_name":"scbrown/tap/bobbin","tap":"scbrown/tap","pinned":%s,"versions":{"stable":"%s"},"installed":[{"version":"%s"}]}]}' "$(cat "$HOME/pinned")" "$(cat "$HOME/local-stable")" "$(cat "$HOME/installed")"; exit 0;;
 "upgrade scbrown/tap/bobbin") cp "$HOME/local-stable" "$HOME/installed"; exit 0;;
 "info --json=v2 scbrown/tap/quipu") printf '{"formulae":[{"full_name":"scbrown/tap/quipu","tap":"scbrown/tap","pinned":false,"versions":{"stable":"0.14.1"},"installed":[{"version":"0.14.1"}]}]}'; exit 0;;
esac
exit 2
'''
BOBBIN='''if [ "${1:-}" = --version ]; then printf 'bobbin %s\\n' "$(cat "$HOME/installed")"; exit 0; fi
if [ "${1:-}" = init ]; then mkdir -p .bobbin; exit 0; fi
if [ "${1:-}" = index ]; then
 if [ -f fixture.rs ]; then cp fixture.rs .bobbin/indexed; else : > .bobbin/indexed; fi
 exit 0
fi
if [ "${1:-}" = grep ]; then
 if grep -q caboodle_verify_marker_ .bobbin/indexed 2>/dev/null; then echo '{"count":1,"results":[{"file_path":"fixture.rs"}]}';else echo '{"count":0,"results":[]}';fi
 exit 0
fi
exit 2
'''
def fake(p,body):p.write_text('#!/bin/sh\nset -eu\n'+body);p.chmod(0o700)
def run(binary,candidate,pinned):
 with tempfile.TemporaryDirectory(prefix='brew-demo-')as raw:
  root=pathlib.Path(raw);b=root/'bin';b.mkdir();fake(b/'brew',BREW);fake(b/'bobbin',BOBBIN);fake(b/'curl',"echo unexpected-network-attempt >&2\nexit 97\n");fake(b/'quipu',"if [ \"${1:-}\" = --version ]; then echo 'quipu 0.14.1'; exit 0; fi\nexit 2\n")
  fake(b/'quipu-server',"echo 'quipu-server 0.14.1'\n")
  stale=root/'cargo/bin';stale.mkdir(parents=True)
  fake(stale/'quipu',"echo 'quipu 0.8.1'\n");fake(stale/'quipu-server',"echo 'quipu-server 0.8.1'\n")
  for name,value in [('installed','0.28.0'),('local-stable','0.28.0'),('remote-stable','0.29.0'),('pinned','true'if pinned else'false'),('caboodle-intent.toml',INTENT)]: (root/name).write_text(value)
  camayoc=root/'camayoc';camayoc.mkdir();(camayoc/'REVISION').write_text('f443bf19974d06ab662c8ae3deeccfbb1e082f48')
  env={k:v for k,v in os.environ.items()if not k.startswith(('QUIPU_AUTH_','GIT_CONFIG_'))}
  env.update(HOME=str(root),PATH=str(b)+os.pathsep+os.environ.get('PATH',''),CARGO_HOME=str(root/'cargo'),CABOODLE_BREW=str(b/'brew'),CABOODLE_CAMAYOC_ROOT=str(camayoc))
  p=subprocess.run([binary,'plan','--profile','retrieval'],cwd=root,env=env,capture_output=True,text=True);assert p.returncode==0,p.stderr

  p=subprocess.run([binary,'update'],cwd=root,env=env,capture_output=True,text=True)
  log=(root/'brew.log').read_text().splitlines()if(root/'brew.log').exists()else[];installed=(root/'installed').read_text().strip()
  print(json.dumps({'binary':'candidate'if candidate else'official0.3.5','pinned':pinned,'exit':p.returncode,'installed':installed,'local_stable':(root/'local-stable').read_text().strip(),'refreshes':log.count('update --quiet'),'upgrades':log.count('upgrade scbrown/tap/bobbin'),'output':p.stdout.strip(),'stderr':p.stderr.strip()}),flush=True)
  if not candidate:
   assert p.returncode!=0 and 'brew receipt and executable version disagree' in p.stderr
   assert installed=='0.28.0'
   return
  assert p.returncode==0,p.stderr
  assert installed=='0.29.0'
  assert log.count('update --quiet')==1
  assert log.count('upgrade scbrown/tap/bobbin')==1
  assert not any('unpin'in line for line in log)
run(sys.argv[1],False,False)
run(sys.argv[2],True,False)
