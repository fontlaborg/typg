/// The gentle puppeteer behind the curtain, pulling the right strings at build time.
///
/// This build script acts like a helpful stage manager for typg-core, making sure
/// everything finds its proper place when the curtain rises. Think of it as the
/// calm coordinator who whispers "you're welcome" to the compiler.
///
/// Currently keeping things simple with a watchful eye on its own changes.
/// Future plans include gracefully handing over prebuilt font library indexes
/// to the main crate - like passing a well-organized recipe card to the chef.

fn main() {
    // Tap dance: Tell Cargo to rebuild when this script changes its moves
    println!("cargo:rerun-if-changed=build.rs");
}
