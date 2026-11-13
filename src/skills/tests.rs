use super::*;
use crate::skill_profile::{CatalogueSnapshot, Harness, Pin, Release, SkillProfile};
use serde_json::json;

#[test]
fn partial_or_wrong_pin_refuses_before_native_execution_or_destination_touch() {
    let temp = tempfile::tempdir().unwrap();
    let profile = SkillProfile {
        id: "urn:fixture:profile".into(),
        public_only: true,
        harnesses: vec![Harness::Claude],
        pins: vec![Pin {
            skill: "urn:fixture:skill".into(),
            version: "1".into(),
            share_id: format!("sha256:{}", "a".repeat(64)),
        }],
    };
    let release = Release {
        pin: profile.pins[0].clone(),
        name: "example".into(),
        render_spec: "urn:fixture:render".into(),
        public: true,
        harnesses: vec![Harness::Claude],
    };
    write_json(&temp.path().join("profile.json"), &profile).unwrap();
    let dest = temp.path().join("dest");
    fs::create_dir(&dest).unwrap();
    fs::write(dest.join("foreign"), b"unchanged").unwrap();
    for (complete, version) in [(false, "1"), (true, "2")] {
        let mut wrong = release.clone();
        wrong.pin.version = version.into();
        write_json(
            &temp.path().join("catalogue.json"),
            &CatalogueSnapshot {
                transaction: 1,
                complete,
                releases: vec![wrong],
            },
        )
        .unwrap();
        let error = sync::sync(
            &temp.path().join("profile.json"),
            &temp.path().join("catalogue.json"),
            &temp.path().join("ABSENT-bundles"),
            &temp.path().join("ABSENT-authority"),
            &temp.path().join("ABSENT-shapes"),
            &dest,
            Path::new("ABSENT-native-program"),
        )
        .unwrap_err();
        assert!(
            error.to_string().contains(if complete {
                "does not satisfy"
            } else {
                "catalogue is unavailable"
            }),
            "{error}"
        );
        assert_eq!(fs::read(dest.join("foreign")).unwrap(), b"unchanged");
        assert_eq!(fs::read_dir(&dest).unwrap().count(), 1);
    }
}

#[test]
fn symlink_and_oversized_inputs_never_become_trusted_skill_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let original = dir.path().join("original");
    fs::write(&original, b"contents").unwrap();
    std::os::unix::fs::symlink(&original, dir.path().join("alias")).unwrap();
    assert!(native::regular_bytes(&dir.path().join("alias"), 100).is_err());
    assert!(native::regular_bytes(&original, 1).is_err());
    assert_eq!(native::regular_bytes(&original, 100).unwrap(), b"contents");
}

#[test]
fn invalid_publish_metadata_cannot_create_output_or_invoke_native_program() {
    let dir = tempfile::tempdir().unwrap();
    let metadata = dir.path().join("metadata.json");
    fs::write(&metadata, serde_json::to_vec(&json!({"skill":"urn:fixture:skill", "name":"../escape",
        "description":"example", "repository":"https://example.test/repo", "source_path":"skills/example",
        "version":"1", "render_spec":"urn:fixture:spec", "public":false,"harnesses":["claude"]})).unwrap()).unwrap();
    let output = dir.path().join("output");
    let error = publish::publish(
        &dir.path().join("ABSENT-source"),
        &metadata,
        &dir.path().join("ABSENT-shapes"),
        None,
        &output,
        Path::new("ABSENT-native-program"),
    )
    .unwrap_err();
    assert!(
        error.to_string().contains("invalid skill release metadata"),
        "{error}"
    );
    assert!(!output.exists());
}
