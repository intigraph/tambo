from pathlib import Path
import os, subprocess, tempfile, re
text=Path('docs/book/src/installing.md').read_text().split('## Replacing a cargo-installed Bobbin')[1].split('## Opt-in published')[0]
prepare,restore=re.findall(r"python3 - <<'PYTHON'\n(.*?)\nPYTHON",text,re.S)
for kind in ['regular','backup-exists','backup-symlink','source-symlink','source-directory']:
 with tempfile.TemporaryDirectory() as d:
  cargo=Path(d)/'cargo';b=cargo/'bin/bobbin';b.parent.mkdir(parents=True);backup=b.with_name('bobbin.before-tambo'); b.write_bytes(b'original cargo binary');b.chmod(0o755)
  if kind=='backup-exists': backup.write_bytes(b'existing backup')
  if kind=='backup-symlink': backup.symlink_to('missing')
  if kind=='source-symlink': b.unlink(); b.symlink_to('missing')
  if kind=='source-directory': b.unlink(); b.mkdir()
  env={**os.environ,'CARGO_HOME':str(cargo)}
  r=subprocess.run(['python3','-c',prepare],env=env,capture_output=True,text=True)
  if kind=='regular':
   assert r.returncode==0 and not b.exists() and backup.read_bytes()==b'original cargo binary'
   assert backup.stat().st_mode & 0o111
   target=Path(d)/'release-bobbin';target.write_bytes(b'reviewed release');b.symlink_to(target)
   assert b.read_bytes()==b'reviewed release'
   fail=subprocess.run(['python3','-c',restore],env=env,capture_output=True)
   assert fail.returncode!=0 and b.is_symlink() and target.read_bytes()==b'reviewed release'
   b.unlink()
   r=subprocess.run(['python3','-c',restore],env=env,capture_output=True,text=True)
   assert r.returncode==0 and b.read_bytes()==backup.read_bytes()==b'original cargo binary'
   print('regular: backup preserved executable bytes; replacement fixture read; occupied rollback refused; empty-path rollback restored original')
  else:
   assert r.returncode!=0 and b.lstat()
   if kind.startswith('backup'): assert b.read_bytes()==b'original cargo binary'
   print(kind+': refused; original preserved')
