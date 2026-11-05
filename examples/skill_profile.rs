//! Offline profile-selection demonstration; no graph or installation is used.
use caboodle::skill_profile::{select, CatalogueSnapshot, Harness, Pin, Release, SkillProfile};

fn main() -> anyhow::Result<()> {
    let snapshot = CatalogueSnapshot {
        transaction: 42,
        complete: true,
        releases: ["public-one", "public-two", "my-skill"]
            .into_iter()
            .enumerate()
            .map(|(index, name)| Release {
                pin: Pin {
                    skill: format!("urn:example:skill:{name}"),
                    share_id: format!("sha256:{index:064x}"),
                    version: "1.0.0".into(),
                },
                name: name.into(),
                render_spec: format!("urn:example:render:{name}"),
                public: true,
                harnesses: vec![Harness::Claude, Harness::Codex],
            })
            .collect(),
    };
    let mut profile = SkillProfile {
        id: "urn:example:my-profile".into(),
        public_only: true,
        harnesses: vec![Harness::Claude, Harness::Codex],
        pins: snapshot.releases.iter().map(|r| r.pin.clone()).collect(),
    };
    let selection = select(&profile, &snapshot)?;
    profile.pins[0].share_id = format!("sha256:{}", "f".repeat(64));
    let stale_refused = select(&profile, &snapshot).is_err();
    anyhow::ensure!(stale_refused, "a stale pin was accepted");
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "selection": selection,
            "stale_pin_refused": stale_refused,
            "target": "synthetic metadata; no files or settings written"
        }))?
    );
    Ok(())
}
