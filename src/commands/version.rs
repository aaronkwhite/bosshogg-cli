use crate::error::Result;
use crate::output;
use serde::Serialize;

#[derive(Serialize)]
struct VersionInfo {
    version: &'static str,
}

pub fn execute(json: bool) -> Result<()> {
    let info = VersionInfo {
        version: env!("CARGO_PKG_VERSION"),
    };
    if json {
        output::print_json(&info);
    } else {
        println!("bosshogg {}", info.version);
    }
    Ok(())
}
