import subprocess, types, tempfile, os
from pathlib import Path
from unittest.mock import patch
base="8755317372950f071508d31c92ae3107a1d9e9d2"
for label,source in [("BASE "+base,subprocess.check_output(["git","show",base+":scripts/macos-job-account-packet.py"]).decode()),("CANDIDATE source",Path("scripts/macos-job-account-packet.py").read_text())]:
 print(label)
 p=types.ModuleType("packet");exec(compile(source,"packet.py","exec"),p.__dict__)
 answer=types.SimpleNamespace(stdout="dsAttrTypeNative:IsHidden: 1\n")
 with patch.object(p,"run",return_value=answer):
  print("input: dsAttrTypeNative:IsHidden: 1 ->",p.properties("/Users/fixture",["IsHidden"]))
 answer.stdout="IsHidden: 1\ndsAttrTypeNative:IsHidden: 1\n"
 with patch.object(p,"run",return_value=answer):
  try:p.properties("/Users/fixture",[]);print("duplicate aliases: ACCEPTED")
  except ValueError:print("duplicate aliases: REFUSED")
 with tempfile.TemporaryDirectory() as root,patch.object(p,"protected"):
  old=os.umask(0o077)
  try:
   d=Path(root)/"packet";p.directory(d);print("umask077 new directory mode:",oct(d.stat().st_mode & 0o777))
  finally:os.umask(old)
