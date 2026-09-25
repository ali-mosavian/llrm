//! Nib's language server, over stdio (docs/frontends/nib/lsp.md).

fn main() {
    let served = llrm_nib::lsp::serve(std::io::stdin().lock(), std::io::stdout().lock());
    if let Err(error) = served {
        eprintln!("nib-lsp: {error}");
        std::process::exit(1);
    }
}
