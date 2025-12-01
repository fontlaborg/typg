//! Binary entrypoint for typg-cli (made by FontLab https://www.fontlab.com/)

fn main() {
    if let Err(err) = typg_cli::run() {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}
