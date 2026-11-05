use super::*;

fn fixture() -> (SkillProfile, CatalogueSnapshot) {
    let releases: Vec<_> = ["one", "two", "own"]
        .into_iter()
        .enumerate()
        .map(|(i, name)| Release {
            pin: Pin {
                skill: format!("urn:example:skill:{name}"),
                share_id: format!("sha256:{i:064x}"),
                version: "1.0.0".into(),
            },
            name: name.into(),
            render_spec: format!("urn:example:render:{name}"),
            public: true,
            harnesses: vec![Harness::Claude, Harness::Codex],
        })
        .collect();
    (
        SkillProfile {
            id: "urn:example:profile".into(),
            public_only: true,
            harnesses: vec![Harness::Codex, Harness::Claude],
            pins: releases.iter().map(|r| r.pin.clone()).collect(),
        },
        CatalogueSnapshot {
            transaction: 42,
            complete: true,
            releases,
        },
    )
}

#[test]
fn three_pins_for_both_harnesses_are_deterministic() {
    let (mut p, mut s) = fixture();
    let first = serde_json::to_value(select(&p, &s).unwrap()).unwrap();
    p.pins.reverse();
    s.releases.reverse();
    p.harnesses.reverse();
    assert_eq!(
        first,
        serde_json::to_value(select(&p, &s).unwrap()).unwrap()
    );
    assert_eq!(first["transaction"], 42);
    assert_eq!(first["releases"].as_array().unwrap().len(), 3);
}

#[test]
fn partial_unknown_and_wrong_pins_do_not_fall_back() {
    for case in 0..11 {
        let (mut p, mut s) = fixture();
        match case {
            0 => s.complete = false,
            1 => s.transaction = 0,
            2 => s.releases.clear(),
            3 => p.pins[0].share_id = format!("sha256:{}", "f".repeat(64)),
            4 => p.pins[0].version = "2.0.0".into(),
            5 => p.pins[0].share_id = format!("sha256:{}", "F".repeat(64)),
            6 => p.pins.clear(),
            7 => p.id = " ".into(),
            8 => p.pins[0].skill = " ".into(),
            9 => p.pins[0].share_id = "0".repeat(64),
            _ => p.pins[0].share_id = format!("SHA256:{}", "0".repeat(64)),
        }
        assert!(select(&p, &s).is_err(), "case {case}");
    }
}

#[test]
fn public_scope_and_harness_support_are_explicit() {
    let (mut p, mut s) = fixture();
    s.releases[0].public = false;
    assert!(select(&p, &s).is_err());
    p.public_only = false;
    assert!(select(&p, &s).is_ok());
    s.releases[0].harnesses = vec![Harness::Claude];
    assert!(select(&p, &s).is_err());
}

#[test]
fn ambiguous_metadata_and_aliases_refuse() {
    for case in 0..6 {
        let (mut p, mut s) = fixture();
        match case {
            0 => s.releases.push(s.releases[0].clone()),
            1 => p.pins.push(p.pins[0].clone()),
            2 => s.releases[0].name = s.releases[1].name.clone(),
            3 => s.releases[0].name = "../escape".into(),
            4 => p.harnesses.push(Harness::Claude),
            _ => s.releases[0].harnesses.clear(),
        }
        assert!(select(&p, &s).is_err(), "case {case}");
    }
}

#[test]
fn unknown_fields_and_harnesses_are_not_silently_dropped() {
    let (p, _) = fixture();
    let mut value = serde_json::to_value(p).unwrap();
    value["fallback"] = true.into();
    assert!(serde_json::from_value::<SkillProfile>(value).is_err());
    assert!(serde_json::from_str::<Harness>("\"arbitrary-path\"").is_err());
}
