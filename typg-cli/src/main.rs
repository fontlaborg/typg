//! Gateway to typg: your friendly font discovery companion.
//! 
//! Think of this as the friendly doorman to the world's fastest font search toolkit.
//! When you type `typg`, you're not just running a program - you're unleashing a 
//! gentle but powerful breeze through your font collection. This main function is simple
//! because the real magic happens in `typg_cli::run()`, where we translate your font-finding
//! wishes into concrete results.
//! 
/// Main entry point that gracefully handles CLI execution.
/// 
/// This function serves as the calm first responder for any font discovery adventures.
/// It runs the main CLI logic and, should anything go sideways, provides a clean exit
/// with a helpful error message. No panics, no drama - just smooth, reliable font searching.
fn main() {
    // Let the main CLI logic do its thing, but catch any hiccups gracefully
    if let Err(err) = typg_cli::run() {
        // Print error to stderr in a friendly, non-alarming way
        eprintln!("error: {err}");
        // Exit with code 1 to signal that our font journey hit a snag
        std::process::exit(1);
    }
}
