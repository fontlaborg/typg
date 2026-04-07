/// Build script for typg-core.
fn main() {
    // Rebuild if this script changes.
    println!("cargo:rerun-if-changed=build.rs");
}
