//! Nib's language server, over stdio (docs/lsp.md).

fn main() {
    let served = llrm::frontends::nib::lsp::serve(std::io::stdin().lock(), std::io::stdout().lock());
    if let Err(error) = served {
        eprintln!("nib-lsp: {error}");
        std::process::exit(1);
    }
}
