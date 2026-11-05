//! `src/primer.md` is the one source for `tambo prime` (stack prime contract).
//!
//! The tambo skill carries a generated copy between markers, and the book
//! includes the file directly. This test fails when the skill's copy drifts;
//! `TAMBO_BLESS_PRIMER=1 cargo test --test prime_sync` rewrites it.

use std::path::{Path, PathBuf};

const BEGIN: &str = "<!-- BEGIN tambo prime";
const END: &str = "<!-- END tambo prime -->";

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

/// The primer as the skill embeds it: every heading one level deeper, so the
/// primer's title nests under the skill's own `## Agent primer`.
fn embedded(primer: &str) -> String {
    primer
        .lines()
        .map(|l| {
            if l.starts_with('#') {
                format!("##{l}")
            } else {
                l.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn skill_carries_the_current_primer() {
    let primer = std::fs::read_to_string(root().join("src/primer.md")).unwrap();
    let skill_path = root().join("skills/tambo/SKILL.md");
    let skill = std::fs::read_to_string(&skill_path).unwrap();
    let start = skill
        .find(BEGIN)
        .expect("skill lost its BEGIN tambo prime marker");
    let body_start = start + skill[start..].find('\n').unwrap() + 1;
    let end = skill
        .find(END)
        .expect("skill lost its END tambo prime marker");
    let want = format!("{}\n", embedded(&primer));
    if skill[body_start..end] == want {
        return;
    }
    if std::env::var_os("TAMBO_BLESS_PRIMER").is_some() {
        let fixed = format!("{}{want}{}", &skill[..body_start], &skill[end..]);
        std::fs::write(&skill_path, fixed).unwrap();
        return;
    }
    panic!(
        "skills/tambo/SKILL.md carries a stale copy of src/primer.md. Run \
         TAMBO_BLESS_PRIMER=1 cargo test --test prime_sync and commit the result."
    );
}

#[test]
fn book_includes_the_primer_and_the_contract() {
    let book = root().join("docs/book/src");
    let page = std::fs::read_to_string(book.join("agent-primer.md")).unwrap();
    assert!(page.contains("{{#include ../../../src/primer.md}}"));
    let contract = std::fs::read_to_string(book.join("prime-contract.md")).unwrap();
    assert!(contract.contains("{{#include ../../stack/PRIME-CONTRACT.md}}"));
    let summary = std::fs::read_to_string(book.join("SUMMARY.md")).unwrap();
    assert!(summary.contains("(agent-primer.md)"));
    assert!(summary.contains("(prime-contract.md)"));
}
