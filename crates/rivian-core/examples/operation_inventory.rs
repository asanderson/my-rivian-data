//! Emit the reviewed native catalog for fingerprint and compiled-WASM parity checks.
fn main() {
    let operations = rivian_core::live::catalog();
    println!(
        "{}",
        serde_json::to_string(&rivian_core::inventory::enrich_catalog(&operations))
            .expect("reviewed catalog serializes")
    );
}
