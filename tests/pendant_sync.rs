#![cfg(unix)]
use assert_cmd::Command;
use predicates::prelude::*;
use std::{
    fs,
    os::unix::fs::{symlink, PermissionsExt},
    path::PathBuf,
    process::Command as Git,
};

struct Fixture {
    temp: tempfile::TempDir,
    repo: PathBuf,
    authority: PathBuf,
    trust: PathBuf,
    db: PathBuf,
    fake: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let repo = root.join("repo");
        fs::create_dir(&repo).unwrap();
        assert!(Git::new("git")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .args(["init", "-q"])
            .arg(&repo)
            .status()
            .unwrap()
            .success());
        assert!(Git::new("git")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .arg("-C")
            .arg(&repo)
            .args([
                "remote",
                "add",
                "origin",
                "https://example.org/owner/project.git"
            ])
            .status()
            .unwrap()
            .success());
        fs::create_dir_all(repo.join(".quipu/packs")).unwrap();
        fs::write(
            repo.join(".quipu/packs/one.pendant.tar.gz"),
            "first pendant",
        )
        .unwrap();
        let authority = root.join("trusted ' authority.pendant.tar.gz");
        fs::write(&authority, "operator shapes").unwrap();
        let trust = root.join("trust.toml");
        fs::write(&trust, "owners = ['example.org/owner']\n").unwrap();
        let db = root.join("local.db");
        let fake = root.join("quipu");
        fs::write(&fake,r#"#!/usr/bin/python3
import json,os,pathlib,sys
a=sys.argv[1:]; root=pathlib.Path(os.environ['FIXTURE_ROOT']); mode=(root/'mode').read_text() if (root/'mode').exists() else 'ok'
with (root/'calls').open('a') as f:f.write(json.dumps(a)+'\n')
assert a[0]=='compose' and a[a.index('--shapes-from')+1]=='_caboodle-authority'
repo=a[a.index('--repo')+1]; db=pathlib.Path(a[a.index('--db')+1]); paths=[s.split('=',1) for s in a if '=' in s]
assert any(k=='_caboodle-authority' and pathlib.Path(p).read_text()=='operator shapes' for k,p in paths)
if mode=='refuse': print('fixture hash mismatch',file=sys.stderr);sys.exit(9)
if mode=='quarantine': print(json.dumps({'repo':repo,'composition':{'outcome':'quarantined'}}));sys.exit(2)
if mode=='empty':sys.exit(0)
if mode=='frozen':
 (root/'repo/.quipu/packs/one.pendant.tar.gz').write_text('concurrent replacement')
 assert dict(paths)['one.pendant.tar.gz'] != str(root/'repo/.quipu/packs/one.pendant.tar.gz')
 assert pathlib.Path(dict(paths)['one.pendant.tar.gz']).read_text()=='first pendant'
db.write_text('composed '+str(len((root/'calls').read_text())))
print(json.dumps({'repo':repo if mode!='wrongrepo' else 'other','composition':{'outcome':'composed'},'current':{'dataset':'urn:quipu:repo:'+repo+':current','members':[k for k,p in paths]}}))
"#).unwrap();
        fs::set_permissions(&fake, fs::Permissions::from_mode(0o755)).unwrap();
        Self {
            temp,
            repo,
            authority,
            trust,
            db,
            fake,
        }
    }
    fn cmd(&self, binary: &str) -> Command {
        let mut c = Command::cargo_bin(binary).unwrap();
        c.current_dir(&self.repo)
            .env("HOME", self.temp.path())
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("QUIPU_BIN", &self.fake)
            .env("FIXTURE_ROOT", self.temp.path())
            .env_remove("QUIPU_AUTH_TOKEN")
            .env_remove("QUIPU_AUTH_TOKEN_FILE");
        c
    }
    fn sync(&self) -> Command {
        let mut c = self.cmd("tambo");
        c.args(["packs", "sync", "--repo"])
            .arg(&self.repo)
            .arg("--shapes-from")
            .arg(&self.authority)
            .arg("--trust-file")
            .arg(&self.trust)
            .arg("--db")
            .arg(&self.db);
        c
    }
    fn receipt(&self) -> PathBuf {
        self.temp.path().join("local.db.caboodle-packs.json")
    }
    fn mode(&self, mode: &str) {
        fs::write(self.temp.path().join("mode"), mode).unwrap();
    }
    fn calls(&self) -> usize {
        fs::read_to_string(self.temp.path().join("calls"))
            .unwrap_or_default()
            .lines()
            .count()
    }
}

#[test]
fn sync_replays_all_members_only_when_needed_and_not_after_removed_trust() {
    let f = Fixture::new();
    f.sync()
        .assert()
        .success()
        .stdout(predicate::str::contains("composed"));
    assert_eq!(f.calls(), 1);
    f.sync()
        .assert()
        .success()
        .stdout(predicate::str::contains("unchanged"));
    assert_eq!(f.calls(), 1);
    fs::write(
        f.repo.join(".quipu/packs/two.pendant.tar.gz"),
        "second pendant",
    )
    .unwrap();
    f.sync().assert().success();
    assert_eq!(f.calls(), 2);
    let calls = fs::read_to_string(f.temp.path().join("calls")).unwrap();
    assert!(calls
        .lines()
        .last()
        .unwrap()
        .contains("one.pendant.tar.gz="));
    assert!(calls
        .lines()
        .last()
        .unwrap()
        .contains("two.pendant.tar.gz="));
    fs::remove_file(f.repo.join(".quipu/packs/one.pendant.tar.gz")).unwrap();
    fs::remove_file(f.repo.join(".quipu/packs/two.pendant.tar.gz")).unwrap();
    f.sync().assert().success();
    assert_eq!(f.calls(), 3);
    fs::write(
        f.repo.join(".quipu/packs/one.pendant.tar.gz"),
        "first pendant",
    )
    .unwrap();
    f.sync().assert().success();
    let receipt = fs::read(f.receipt()).unwrap();
    fs::write(&f.trust, "owners=[]\n").unwrap();
    f.sync()
        .assert()
        .failure()
        .stderr(predicate::str::contains("has no allow entry"));
    assert_eq!(f.calls(), 4);
    assert_eq!(fs::read(f.receipt()).unwrap(), receipt);
}

#[test]
fn refusal_quarantine_and_unattested_success_never_replace_prior_receipt() {
    for mode in ["refuse", "quarantine", "empty", "wrongrepo"] {
        let f = Fixture::new();
        f.sync().assert().success();
        let receipt = fs::read(f.receipt()).unwrap();
        fs::write(f.repo.join(".quipu/packs/one.pendant.tar.gz"), "changed").unwrap();
        f.mode(mode);
        f.sync().assert().failure();
        assert_eq!(fs::read(f.receipt()).unwrap(), receipt);
        // A wrong-repo success can have mutated the fixture DB; the caller must not
        // label that response loaded or retry it automatically.
        assert_eq!(f.calls(), 2);
    }
}

#[test]
fn cache_is_invalidated_by_database_removal_or_wal_change() {
    let f = Fixture::new();
    f.sync().assert().success();
    fs::remove_file(&f.db).unwrap();
    f.sync().assert().success();
    assert_eq!(f.calls(), 2);
    fs::write(f.temp.path().join("local.db-wal"), "external mutation").unwrap();
    f.sync().assert().success();
    assert_eq!(f.calls(), 3);
}

#[test]
fn trust_and_composition_consume_the_same_frozen_archive() {
    let f = Fixture::new();
    f.mode("frozen");
    f.sync().assert().success();
    assert_eq!(f.calls(), 1);
}

#[test]
fn no_packs_is_a_noop_but_missing_authority_and_symlink_inputs_refuse() {
    let f = Fixture::new();
    f.cmd("caboodle")
        .args(["packs", "sync"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("requires --shapes-from"));
    assert_eq!(f.calls(), 0);
    fs::remove_file(f.repo.join(".quipu/packs/one.pendant.tar.gz")).unwrap();
    f.cmd("caboodle")
        .args(["packs", "sync"])
        .assert()
        .success()
        .stdout(predicate::str::contains("no_packs"));
    assert!(!f.db.exists());
    symlink(
        &f.authority,
        f.repo.join(".quipu/packs/link.pendant.tar.gz"),
    )
    .unwrap();
    f.sync()
        .assert()
        .failure()
        .stderr(predicate::str::contains("regular file"));
    assert_eq!(f.calls(), 0);
}

#[test]
fn hook_opt_in_is_idempotent_preserves_existing_hooks_and_executes_sync() {
    let f = Fixture::new();
    let hooks = f.repo.join(".git/hooks");
    f.cmd("tambo").arg("init").assert().failure();
    assert!(!hooks.join("post-checkout").exists());
    for _ in 0..2 {
        f.cmd("tambo")
            .args(["init", "--pack-hooks", "--repo"])
            .arg(&f.repo)
            .arg("--shapes-from")
            .arg(&f.authority)
            .arg("--db")
            .arg(&f.db)
            .arg("--trust-file")
            .arg(&f.trust)
            .assert()
            .success();
    }
    assert!(Git::new("git")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .arg("-C")
        .arg(&f.repo)
        .args([
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.org",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "--allow-empty",
            "-qm",
            "fixture"
        ])
        .status()
        .unwrap()
        .success());
    assert!(Git::new("git")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .arg("-C")
        .arg(&f.repo)
        .args(["checkout", "-qb", "hook-control"])
        .current_dir(&f.repo)
        .env("HOME", f.temp.path())
        .env("QUIPU_BIN", &f.fake)
        .env("FIXTURE_ROOT", f.temp.path())
        .status()
        .unwrap()
        .success());
    assert_eq!(f.calls(), 1);
    fs::write(hooks.join("post-merge"), "existing operator hook").unwrap();
    f.cmd("tambo")
        .args(["init", "--pack-hooks", "--shapes-from"])
        .arg(&f.authority)
        .assert()
        .failure()
        .stderr(predicate::str::contains("existing hook retained"));
    assert_eq!(
        fs::read_to_string(hooks.join("post-merge")).unwrap(),
        "existing operator hook"
    );
}

#[test]
fn configured_hook_directory_is_never_taken_over() {
    let f = Fixture::new();
    assert!(Git::new("git")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .arg("-C")
        .arg(&f.repo)
        .args(["config", "core.hooksPath", "shared-hooks"])
        .status()
        .unwrap()
        .success());
    f.cmd("tambo")
        .args(["init", "--pack-hooks", "--shapes-from"])
        .arg(&f.authority)
        .assert()
        .failure()
        .stderr(predicate::str::contains("core.hooksPath"));
    assert!(!f.repo.join("shared-hooks").exists());
}

#[test]
fn a_natural_post_merge_event_syncs_the_new_archive() {
    let f = Fixture::new();
    let git = |args: &[&str]| {
        let output = Git::new("git")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .arg("-C")
            .arg(&f.repo)
            .args([
                "-c",
                "user.name=Fixture",
                "-c",
                "user.email=fixture@example.org",
                "-c",
                "commit.gpgsign=false",
            ])
            .args(args)
            .env("HOME", f.temp.path())
            .env("QUIPU_BIN", &f.fake)
            .env("FIXTURE_ROOT", f.temp.path())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap().trim().to_string()
    };
    git(&["add", ".quipu"]);
    git(&["commit", "-qm", "base"]);
    let base = git(&["branch", "--show-current"]);
    git(&["checkout", "-qb", "pendant-update"]);
    fs::write(
        f.repo.join(".quipu/packs/one.pendant.tar.gz"),
        "new committed pendant",
    )
    .unwrap();
    git(&["add", ".quipu"]);
    git(&["commit", "-qm", "updated pendant"]);
    git(&["checkout", "-q", &base]);
    f.cmd("tambo")
        .args(["init", "--pack-hooks", "--shapes-from"])
        .arg(&f.authority)
        .arg("--db")
        .arg(&f.db)
        .arg("--trust-file")
        .arg(&f.trust)
        .assert()
        .success();
    f.sync().assert().success();
    assert_eq!(f.calls(), 1);
    git(&["merge", "--ff-only", "-q", "pendant-update"]);
    assert_eq!(f.calls(), 2);
    assert_eq!(
        fs::read_to_string(f.repo.join(".quipu/packs/one.pendant.tar.gz")).unwrap(),
        "new committed pendant"
    );
}
