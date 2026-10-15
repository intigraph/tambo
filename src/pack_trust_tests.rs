use super::*;

const SHA: &str = "4f2d1c0b9a8e7d6c5b4a39281706f5e4d3c2b1a0f9e8d7c6b5a4938271605f4e";
const OTHER: &str = "0000000000000000000000000000000000000000000000000000000000000001";

fn own() -> TrustConfig {
    TrustConfig {
        owners: vec!["github.com/scbrown".into()],
        ..Default::default()
    }
}

#[test]
fn a_foreign_repository_with_no_allow_entry_is_refused_naming_the_pack() {
    let decision = own().decide_pack(
        "https://github.com/someone/else.git",
        ".quipu/packs/main.pendant.tar.gz",
        SHA,
    );
    let Decision::Refuse { message } = decision else {
        panic!("a foreign pack must not load without an allow entry");
    };
    assert!(
        message.contains(".quipu/packs/main.pendant.tar.gz"),
        "{message}"
    );
    assert!(message.contains("github.com/someone/else"), "{message}");
    assert!(message.contains("caboodle packs trust allow"), "{message}");
}

#[test]
fn own_repositories_autoload_in_every_remote_form() {
    let trust = own();
    for remote in [
        "https://github.com/scbrown/quipu.git",
        "https://github.com/SCBrown/quipu",
        "git@github.com:scbrown/quipu.git",
        "ssh://git@github.com/scbrown/quipu",
        "github.com/scbrown/quipu",
    ] {
        assert!(trust.decide_pack(remote, "p", SHA).allowed(), "{remote}");
    }
    assert!(
        !trust
            .decide_pack("https://gitlab.com/scbrown/quipu", "p", SHA)
            .allowed(),
        "the owner is host-qualified: the same name elsewhere is someone else"
    );
}

#[test]
fn a_foreign_allow_admits_exactly_one_archive() {
    let mut trust = own();
    assert!(trust
        .add_pack("https://github.com/other/kb.git", SHA, Some("reviewed"))
        .unwrap());
    assert!(
        !trust
            .add_pack("github.com/other/kb", &SHA.to_uppercase(), None)
            .unwrap(),
        "idempotent"
    );
    assert!(trust
        .decide_pack("git@github.com:other/kb.git", "p", SHA)
        .allowed());
    assert!(
        !trust
            .decide_pack("github.com/other/kb", "p", OTHER)
            .allowed(),
        "a new commit of an allowed foreign pack needs its own allow"
    );
    assert!(
        !trust
            .decide_pack("github.com/other/kb2", "p", SHA)
            .allowed(),
        "the same bytes from another repository are another decision"
    );
}

#[test]
fn a_repository_name_without_a_hash_is_not_consent() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("pack-trust.toml");
    fs::write(&path, "[[allow]]\nrepo = \"github.com/other/kb\"\n").unwrap();
    let err = TrustConfig::load(&path).unwrap_err().to_string();
    assert!(err.contains("pack_sha256"), "{err}");
    fs::write(
        &path,
        "[[allow]]\nrepo = \"github.com/other/kb\"\npack_sha256 = \"abc\"\n",
    )
    .unwrap();
    assert!(
        TrustConfig::load(&path).is_err(),
        "a malformed hash refuses the whole file"
    );
    fs::write(
        &path,
        "owners = [\"github.com/scbrown\"]\ntrust_everything = true\n",
    )
    .unwrap();
    assert!(
        TrustConfig::load(&path).is_err(),
        "unknown keys are refused, never ignored"
    );
}

#[test]
fn a_missing_file_trusts_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let trust = TrustConfig::load(&dir.path().join("absent.toml")).unwrap();
    assert!(!trust
        .decide_pack("github.com/scbrown/quipu", "p", SHA)
        .allowed());
}

#[test]
fn foreign_endpoints_need_their_own_allow_and_carry_no_credentials() {
    let mut trust = own();
    let endpoint = "https://quipu.example.org/sparql";
    assert!(trust
        .decide_endpoint("github.com/scbrown/ops", endpoint)
        .allowed());
    let Decision::Refuse { message } = trust.decide_endpoint("github.com/other/kb", endpoint)
    else {
        panic!("declaring an endpoint in a foreign pack must not authorise it");
    };
    assert!(
        message.contains(endpoint) && message.contains("allow-endpoint"),
        "{message}"
    );
    trust
        .add_endpoint("github.com/other/kb", endpoint, None)
        .unwrap();
    assert!(trust
        .decide_endpoint("github.com/other/kb", endpoint)
        .allowed());
    assert!(!trust
        .decide_endpoint("github.com/other/kb", "https://evil.example/sparql")
        .allowed());
    assert!(trust
        .add_endpoint("github.com/other/kb", "https://u:p@host/sparql", None)
        .is_err());
    assert!(trust
        .add_endpoint("github.com/other/kb", "file:///etc/passwd", None)
        .is_err());
}

#[test]
fn save_round_trips_and_reads_back() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("nested/pack-trust.toml");
    let mut trust = own();
    trust.add_owner("GitHub.com/Other-Org").unwrap();
    trust.add_pack("github.com/x/y", SHA, Some("why")).unwrap();
    trust
        .add_endpoint("github.com/x/y", "https://q.example.org/sparql", None)
        .unwrap();
    trust.save(&path).unwrap();
    let loaded = TrustConfig::load(&path).unwrap();
    assert_eq!(loaded, trust);
    assert!(loaded
        .decide_pack("github.com/other-org/z", "p", SHA)
        .allowed());
}

#[test]
fn malformed_inputs_refuse_rather_than_guess() {
    let trust = own();
    for remote in [
        "",
        "github.com/scbrown",
        "https://github.com/a/b/c",
        "ftp://github.com/a/b",
        "github.com/a b/c",
    ] {
        assert!(!trust.decide_pack(remote, "p", SHA).allowed(), "{remote:?}");
    }
    assert!(!trust
        .decide_pack("github.com/scbrown/quipu", "p", "not-a-hash")
        .allowed());
    assert!(
        TrustConfig::default().add_owner("scbrown").is_err(),
        "owner needs a host"
    );
}
