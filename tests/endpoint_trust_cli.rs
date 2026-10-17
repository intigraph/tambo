use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;

#[test]
fn endpoint_decisions_use_current_policy_for_both_binary_names() {
    for binary in ["tambo", "caboodle"] {
        let root = tempfile::tempdir().unwrap();
        let policy = root.path().join("trust.toml");
        let endpoint = "https://example.org/query";
        let repo = "https://github.com/foreign/project.git";
        let check = |url: &str| {
            let mut cmd = Command::cargo_bin(binary).unwrap();
            cmd.env("HOME", root.path())
                .args(["packs", "trust", "--file"])
                .arg(&policy)
                .args(["check-endpoint", repo, url]);
            cmd
        };
        // No file means default deny; checking must not create policy.
        check(endpoint)
            .assert()
            .code(3)
            .stderr(predicate::str::contains(endpoint));
        assert!(!policy.exists());
        let mut trust = caboodle::pack_trust::TrustConfig::default();
        trust
            .add_pack("github.com/foreign/project", &"a".repeat(64), None)
            .unwrap();
        trust.save(&policy).unwrap();
        check(endpoint).assert().code(3);
        trust
            .add_endpoint("github.com/foreign/project", endpoint, None)
            .unwrap();
        trust.save(&policy).unwrap();
        let before = fs::read(&policy).unwrap();
        check(endpoint)
            .assert()
            .success()
            .stdout(predicate::str::contains("federate:"));
        check("https://example.org/other").assert().code(3);
        check("https://user:secret@example.org/query")
            .assert()
            .code(3);
        assert_eq!(fs::read(&policy).unwrap(), before);
        // A later revocation applies immediately.
        trust.endpoints.clear();
        trust.save(&policy).unwrap();
        check(endpoint).assert().code(3);
        trust.add_owner("github.com/foreign").unwrap();
        trust.save(&policy).unwrap();
        check(endpoint).assert().success();
        fs::write(&policy, "unknown_key = true\n").unwrap();
        check(endpoint).assert().failure();
        assert_eq!(fs::read_to_string(&policy).unwrap(), "unknown_key = true\n");
    }
}
