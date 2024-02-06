use std::env;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

fn main() -> std::io::Result<()> {
    // Produce an sync version from the async one
    println!("cargo:rerun-if-changed=src/async.rs");
    let asynced = std::fs::read_to_string("src/async.rs")?;

    let asynced = asynced.replace("embedded_hal_async", "embedded_hal");
    let asynced = asynced.replace("async", "");
    let asynced = asynced.replace(".await", "");

    let mut out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    out_path.push("de-asynced.rs");

    File::create(out_path)?.write_all(asynced.as_bytes())?;

    Ok(())
}
