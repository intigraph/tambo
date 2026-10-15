//! Published Chaski wheel: actual isolated install and reader-path proof.
//! Run with `cargo test --test chaski_member_live -- --ignored`.
#![cfg(unix)]

use std::{env, fs, os::unix::fs::PermissionsExt};

use caboodle::adapter::adapter;
use caboodle::model::{QuipuFlavor, ToolName};

#[test]
#[ignore = "downloads the published Chaski wheel and PyYAML"]
fn chaski_installs_three_programs_and_reads_back_a_real_demo() {
    let root = tempfile::tempdir().unwrap();
    let cargo_home = root.path().join("cargo-home");
    env::set_var("CARGO_HOME", &cargo_home);
    env::set_var("HOME", root.path());
    let system = env::var("PATH").unwrap();
    env::set_var(
        "PATH",
        format!("{}:{}", cargo_home.join("bin").display(), system),
    );

    let member = ToolName::parse("chaski").expect("Chaski is an embedded member");
    let chaski = adapter(member, QuipuFlavor::Release);
    chaski
        .install()
        .expect("install the pinned published wheel");
    for program in ["chaski", "chaski-event", "chaski-once"] {
        assert!(
            cargo_home.join("bin").join(program).is_file(),
            "missing {program}"
        );
    }
    let version = chaski.version().expect("version read-back");
    assert!(
        chaski.is_current(&version),
        "unexpected installed version {version:?}"
    );
    chaski
        .verify()
        .expect("absent -> demo run -> independent status read-back");

    // A successful exit from a writer that does nothing must not prove a run.
    // Replace only this test's own launcher, keeping help and status real.
    let installed = cargo_home.join("bin/chaski");
    let real = root.path().join("real-chaski");
    fs::rename(&installed, &real).unwrap();
    let quoted = format!("'{}'", real.to_str().unwrap().replace('\'', "'\"'\"'"));
    fs::write(&installed, format!(
        "#!/bin/sh\nif [ \"${{1:-}}\" = demo ] && [ \"${{2:-}}\" = run ]; then exit 0; fi\nexec {quoted} \"$@\"\n"
    )).unwrap();
    fs::set_permissions(&installed, fs::Permissions::from_mode(0o755)).unwrap();
    let refused = format!("{:#}", chaski.verify().expect_err("no-op writer must fail"));
    assert!(
        refused.contains("not present after it was created"),
        "{refused}"
    );
    env::set_var("PATH", system);
}
