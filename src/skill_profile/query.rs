//! Bounded native query planning and decoding, without a transport or writes.
//! The eventual transport must bind a response to this exact request; parsing
//! alone cannot attest the server, selected dataset or loaded shape authority.
use super::{select, CatalogueSnapshot, Harness, Pin, Release, Selection, SkillProfile};
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

const NS: &str = "urn:quipu:skills:";
const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";
const STRING: &str = "http://www.w3.org/2001/XMLSchema#string";
const BOOLEAN: &str = "http://www.w3.org/2001/XMLSchema#boolean";
const MAX_ROWS: usize = 8192;

/// A single explicit-dataset, transaction-pinned SELECT request.
#[derive(Debug)]
pub struct Plan {
    profile: String,
    transaction: u64,
    request: Value,
}

fn iri(value: &str) -> bool {
    let Some((scheme, rest)) = value.split_once(':') else {
        return false;
    };
    !rest.is_empty()
        && value.len() <= 2048
        && scheme
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphabetic)
        && scheme
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"+-.".contains(&b))
        && !value
            .chars()
            .any(|c| c.is_whitespace() || c.is_control() || "<>\"{}|^`\\".contains(c))
}

/// Construct data, never execute it. Inputs cannot inject SPARQL syntax.
pub fn plan(profile: &str, graph: &str, transaction: u64) -> Result<Plan> {
    if !iri(profile) || !iri(graph) || transaction == 0 || transaction > i64::MAX as u64 {
        bail!("invalid explicit catalogue query pin");
    }
    // Collect all properties rather than inner-joining required fields: a
    // malformed selected pin must remain visible and refuse, never disappear.
    let query = format!(
        r#"PREFIX s: <{NS}>
SELECT ?record ?subject ?predicate (STR(?value) AS ?text)
 (IF(isIRI(?value), "iri", IF(isLiteral(?value), "literal", "blank")) AS ?kind)
 (IF(isLiteral(?value), STR(DATATYPE(?value)), "") AS ?datatype)
FROM <{graph}>
WHERE {{
 {{ BIND(<{profile}> AS ?subject) ?subject ?predicate ?value . BIND("profile" AS ?record) }}
 UNION {{ <{profile}> s:pin ?subject . ?subject ?predicate ?value . BIND("pin" AS ?record) }}
 UNION {{ <{profile}> s:pin ?pin . ?pin s:skill ?identity ; s:shareId ?share ; s:version ?version .
 ?subject a s:Release ; s:skill ?identity ; s:shareId ?share ; s:version ?version ; ?predicate ?value . BIND("release" AS ?record) }}
 FILTER(isIRI(?subject))
}} LIMIT 8193"#
    );
    Ok(Plan {
        profile: profile.into(),
        transaction,
        request: json!({"query": query, "tx": transaction, "verbose": true}),
    })
}

type Object = (String, String, String); // kind, lexical value, datatype
#[derive(Default)]
struct Record(BTreeMap<String, BTreeSet<Object>>);
impl Record {
    fn values(&self, property: &str, kind: &str, datatype: &str) -> Result<Vec<String>> {
        let key = if property == RDF_TYPE {
            property.to_owned()
        } else {
            format!("{NS}{property}")
        };
        self.0
            .get(&key)
            .context("required catalogue property missing")?
            .iter()
            .map(|(k, v, d)| {
                if k != kind || d != datatype || (kind == "iri" && !iri(v)) {
                    bail!("wrong catalogue RDF term kind");
                }
                Ok(v.clone())
            })
            .collect()
    }
    fn one(&self, property: &str, kind: &str, datatype: &str) -> Result<String> {
        let values = self.values(property, kind, datatype)?;
        if values.len() != 1 {
            bail!("ambiguous catalogue property");
        }
        Ok(values[0].clone())
    }
    fn typed(&self, class: &str) -> Result<()> {
        if !self
            .values(RDF_TYPE, "iri", "")?
            .contains(&format!("{NS}{class}"))
        {
            bail!("catalogue record has no explicit required type");
        }
        Ok(())
    }
    fn pin(&self) -> Result<Pin> {
        Ok(Pin {
            skill: self.one("skill", "iri", "")?,
            share_id: self.one("shareId", "literal", STRING)?,
            version: self.one("version", "literal", STRING)?,
        })
    }
    fn harnesses(&self) -> Result<Vec<Harness>> {
        self.values("harness", "literal", STRING)?
            .iter()
            .map(|s| match s.as_str() {
                "claude" => Ok(Harness::Claude),
                "codex" => Ok(Harness::Codex),
                _ => anyhow::bail!("unsupported catalogue harness"),
            })
            .collect()
    }
}
impl Plan {
    /// Native Quipu query arguments, without credentials or a server endpoint.
    pub fn request(&self) -> &Value {
        &self.request
    }

    /// Decode only a complete, bounded response belonging to this request.
    /// Transport provenance remains the caller's obligation; no I/O occurs.
    pub fn decode(&self, bytes: &[u8]) -> Result<Selection> {
        if bytes.len() > 4 * 1024 * 1024 {
            bail!("catalogue response exceeds byte budget");
        }
        let response: Value = serde_json::from_slice(bytes)?;
        let rows = response["rows"]
            .as_array()
            .context("catalogue response has no rows")?;
        if response["truncated"] != false
            || response["count"].as_u64() != Some(rows.len() as u64)
            || rows.len() > MAX_ROWS
        {
            bail!("catalogue response coverage is unknown");
        }
        let expected: BTreeSet<_> = ["record", "subject", "predicate", "text", "kind", "datatype"]
            .into_iter()
            .collect();
        let variables: BTreeSet<_> = response["variables"]
            .as_array()
            .context("missing catalogue columns")?
            .iter()
            .map(|v| v.as_str().context("invalid catalogue column"))
            .collect::<Result<_>>()?;
        if variables != expected {
            bail!("unexpected catalogue query columns");
        }
        let mut records: BTreeMap<(String, String), Record> = BTreeMap::new();
        for row in rows {
            let field = |key: &str| {
                row[key]
                    .as_str()
                    .context("malformed catalogue row")
                    .map(str::to_owned)
            };
            let record = field("record")?;
            let subject = field("subject")?;
            let predicate = field("predicate")?;
            if !["profile", "pin", "release"].contains(&record.as_str())
                || !iri(&subject)
                || !iri(&predicate)
            {
                bail!("invalid catalogue record identity");
            }
            records
                .entry((record, subject))
                .or_default()
                .0
                .entry(predicate)
                .or_default()
                .insert((field("kind")?, field("text")?, field("datatype")?));
        }
        let profile = records
            .remove(&("profile".into(), self.profile.clone()))
            .context("selected profile absent")?;
        profile.typed("Profile")?;
        let public_only = match profile.one("publicOnly", "literal", BOOLEAN)?.as_str() {
            "true" | "1" => true,
            "false" | "0" => false,
            _ => bail!("invalid public-only policy"),
        };
        let mut pins = Vec::new();
        for id in profile.values("pin", "iri", "")? {
            let pin = records
                .remove(&("pin".into(), id))
                .context("selected pin absent")?;
            pin.typed("Pin")?;
            pins.push(pin.pin()?);
        }
        let selected_keys: BTreeSet<_> = pins
            .iter()
            .map(|p| (&p.skill, &p.share_id, &p.version))
            .collect();
        let mut releases = Vec::new();
        for ((kind, _), record) in records {
            if kind != "release" {
                bail!("unexpected or unselected catalogue record");
            }
            record.typed("Release")?;
            let pin = record.pin()?;
            if !selected_keys.contains(&(&pin.skill, &pin.share_id, &pin.version)) {
                bail!("unselected catalogue release");
            }
            let public = match record.one("scope", "literal", STRING)?.as_str() {
                "public" => true,
                "internal" => false,
                _ => bail!("unsupported catalogue scope"),
            };
            releases.push(Release {
                pin,
                name: record.one("name", "literal", STRING)?,
                render_spec: record.one("renderSpec", "iri", "")?,
                public,
                harnesses: record.harnesses()?,
            });
        }
        let profile = SkillProfile {
            id: self.profile.clone(),
            public_only,
            harnesses: profile.harnesses()?,
            pins,
        };
        select(
            &profile,
            &CatalogueSnapshot {
                transaction: self.transaction,
                complete: true,
                releases,
            },
        )
    }
}

#[cfg(test)]
mod tests;
