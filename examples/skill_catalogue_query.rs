//! Offline request/response demonstration; it performs no transport or writes.
use anyhow::{bail, Context, Result};
use caboodle::skill_profile::query::plan;
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if !(args.len() == 4 || args.len() == 5) {
        bail!("usage: skill_catalogue_query plan|decode PROFILE GRAPH TX [response.json]");
    }
    let p = plan(&args[1], &args[2], args[3].parse()?)?;
    let result = match args[0].as_str() {
        "plan" if args.len() == 4 => p.request().clone(),
        "decode" if args.len() == 5 => {
            if std::fs::metadata(&args[4])?.len() > 4 * 1024 * 1024 {
                bail!("response exceeds byte budget");
            }
            serde_json::to_value(
                p.decode(&std::fs::read(&args[4]).context("read offline response")?)?,
            )?
        }
        _ => bail!("invalid offline demonstration mode"),
    };
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}
