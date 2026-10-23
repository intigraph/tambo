use super::*;

fn fixture() -> Value {
    let mut rows = Vec::new();
    let mut add = |record: &str,
                   subject: &str,
                   predicate: &str,
                   value: &str,
                   kind: &str,
                   datatype: &str| {
        rows.push(json!({"record":record,"subject":subject,"predicate":predicate,"text":value,"kind":kind,"datatype":datatype}));
    };
    add(
        "profile",
        "urn:profile",
        RDF_TYPE,
        &format!("{NS}Profile"),
        "iri",
        "",
    );
    add(
        "profile",
        "urn:profile",
        &format!("{NS}publicOnly"),
        "true",
        "literal",
        BOOLEAN,
    );
    for harness in ["claude", "codex"] {
        add(
            "profile",
            "urn:profile",
            &format!("{NS}harness"),
            harness,
            "literal",
            STRING,
        );
    }
    for name in ["one", "two", "own"] {
        let pin = format!("urn:pin:{name}");
        let release = format!("urn:release:{name}");
        let skill = format!("urn:skill:{name}");
        add(
            "profile",
            "urn:profile",
            &format!("{NS}pin"),
            &pin,
            "iri",
            "",
        );
        for (kind, id, class) in [("pin", &pin, "Pin"), ("release", &release, "Release")] {
            add(kind, id, RDF_TYPE, &format!("{NS}{class}"), "iri", "");
            add(kind, id, &format!("{NS}skill"), &skill, "iri", "");
            add(
                kind,
                id,
                &format!("{NS}version"),
                "1.0.0",
                "literal",
                STRING,
            );
            add(
                kind,
                id,
                &format!("{NS}shareId"),
                &format!("sha256:{}", "1".repeat(64)),
                "literal",
                STRING,
            );
        }
        add(
            "release",
            &release,
            &format!("{NS}name"),
            name,
            "literal",
            STRING,
        );
        add(
            "release",
            &release,
            &format!("{NS}renderSpec"),
            &format!("urn:render:{name}"),
            "iri",
            "",
        );
        add(
            "release",
            &release,
            &format!("{NS}scope"),
            "public",
            "literal",
            STRING,
        );
        for harness in ["claude", "codex"] {
            add(
                "release",
                &release,
                &format!("{NS}harness"),
                harness,
                "literal",
                STRING,
            );
        }
    }
    json!({"variables":["record","subject","predicate","text","kind","datatype"],"count":rows.len(),"truncated":false,"rows":rows})
}
fn selected(value: &Value) -> Result<Selection> {
    plan("urn:profile", "urn:catalogue", 42)?.decode(&serde_json::to_vec(value)?)
}
#[test]
fn exact_plan_and_complete_three_skill_result() {
    let p = plan("urn:profile", "urn:catalogue", 42).unwrap();
    assert!(p.request()["query"]
        .as_str()
        .unwrap()
        .contains("FROM <urn:catalogue>"));
    assert_eq!(p.request()["tx"], 42);
    assert_eq!(p.request()["verbose"], true);
    let mut value = fixture();
    assert_eq!(selected(&value).unwrap().releases.len(), 3);
    let first = serde_json::to_value(selected(&value).unwrap()).unwrap();
    value["rows"].as_array_mut().unwrap().reverse();
    assert_eq!(
        first,
        serde_json::to_value(selected(&value).unwrap()).unwrap()
    );
}
#[test]
fn unknown_partial_and_saturated_results_refuse() {
    for case in 0..6 {
        let mut v = fixture();
        match case {
            0 => v["truncated"] = true.into(),
            1 => {
                v.as_object_mut().unwrap().remove("truncated");
            }
            2 => v["count"] = 0.into(),
            3 => v["rows"] = json!([]),
            4 => v["variables"] = json!(["s"]),
            _ => {
                let row = v["rows"][0].clone();
                v["rows"] = json!(vec![row; MAX_ROWS + 1]);
                v["count"] = (MAX_ROWS + 1).into();
            }
        }
        assert!(selected(&v).is_err(), "case {case}");
    }
}
#[test]
fn missing_pin_metadata_cannot_disappear_in_a_join() {
    let mut v = fixture();
    v["rows"].as_array_mut().unwrap().retain(|r| {
        !(r["record"] == "pin"
            && r["subject"] == "urn:pin:one"
            && r["predicate"] == format!("{NS}shareId"))
    });
    v["count"] = v["rows"].as_array().unwrap().len().into();
    assert!(selected(&v).is_err());
}
#[test]
fn wrong_terms_scope_duplicates_and_unselected_records_refuse() {
    for case in 0..5 {
        let mut v = fixture();
        let rows = v["rows"].as_array_mut().unwrap();
        match case {
            0 => rows[1]["datatype"] = STRING.into(),
            1 => {
                let r = rows
                    .iter_mut()
                    .find(|r| r["record"] == "release" && r["predicate"] == format!("{NS}scope"))
                    .unwrap();
                r["text"] = "internal".into();
            }
            2 => {
                let mut r = rows[1].clone();
                r["text"] = "false".into();
                rows.push(r);
            }
            3 => {
                let mut r = rows[0].clone();
                r["subject"] = "urn:other-profile".into();
                rows.push(r);
            }
            _ => rows[0]["kind"] = "literal".into(),
        }
        v["count"] = v["rows"].as_array().unwrap().len().into();
        assert!(selected(&v).is_err(), "case {case}");
    }
}
#[test]
fn injection_and_invalid_transaction_pins_refuse_before_execution() {
    for id in [
        "relative",
        "urn:x> } UNION { ?s ?p ?o",
        "urn:x\\u003e",
        "urn:x\n",
        "urn:",
        "1bad:x",
    ] {
        assert!(plan(id, "urn:catalogue", 42).is_err());
        assert!(plan("urn:profile", id, 42).is_err());
    }

    assert!(plan("urn:profile", "urn:catalogue", 0).is_err());
    assert!(plan("urn:profile", "urn:catalogue", i64::MAX as u64 + 1).is_err());
}

#[test]
fn observed_native_response_round_trips() {
    let bytes = include_bytes!("../../../tests/fixtures/skill-catalogue-response.json");
    let selection = plan("urn:profile", "urn:quipu:graph:root", 1)
        .unwrap()
        .decode(bytes)
        .unwrap();
    assert_eq!(selection.releases.len(), 3);
    assert_eq!(selection.harnesses, vec![Harness::Claude, Harness::Codex]);
}
