//! Frontend for the provisional modern systems language.
//!
//! Syntax is private to this crate. Successful compilation crosses the
//! process boundary as typed, name-resolved common HIR.

pub mod arguments;
pub mod compile;
pub mod consts;
pub mod conversions;
pub mod declarations;
pub mod desugar;
pub mod driver;
pub mod error;
pub mod hir;
pub mod lexer;
pub mod library;
pub mod main;
pub mod modernstages;
pub mod modules;
pub mod parser;
pub mod semantic;
pub mod syntax;

#[cfg(test)]
mod test_language;
#[cfg(test)]
pub(crate) mod test_modern_frontend;

pub use error::Diagnostic;
pub use lexer::lex;
pub use parser::parse;

pub fn compile(source: &str, module_name: &str) -> Result<String, Diagnostic> {
    let tokens = lex(source)?;
    compile_module(parse(tokens)?, module_name)
}

/// `source`'s tokens, one `line:column kind` per line.
pub fn tokens_text(source: &str) -> Result<String, Diagnostic> {
    Ok(lex(source)?.iter().map(|token| format!("{}:{} {:?}\n", token.span.line, token.span.column, token.kind)).collect())
}

/// `source`'s syntax tree.
pub fn syntax_text(source: &str) -> Result<String, Diagnostic> {
    Ok(format!("{:#?}\n", parse(lex(source)?)?))
}

/// The program whose main module is the file `path`: its imports are the
/// files under the same directory, `a.b` at `a/b.mod`.
pub fn compile_file(path: &std::path::Path) -> Result<String, (std::path::PathBuf, Diagnostic)> {
    let module = load_file(path)?;
    compile_module(module, module_name(path)).map_err(|error| (path.to_path_buf(), error))
}

/// The `.H`, `.BI` or `.INC` declarations of the program at `path`'s exports.
pub fn declare_file(
    path: &std::path::Path,
    language: declarations::Language,
) -> Result<String, (std::path::PathBuf, Diagnostic)> {
    let module = load_file(path)?;
    declarations::declarations(&module, module_name(path), language).map_err(|error| (path.to_path_buf(), error))
}

fn module_name(path: &std::path::Path) -> &str {
    path.file_stem().and_then(|one| one.to_str()).unwrap_or("module")
}

/// The module at `path`, linked with every module it imports.
fn load_file(path: &std::path::Path) -> Result<syntax::Module, (std::path::PathBuf, Diagnostic)> {
    let root = path.parent().unwrap_or(std::path::Path::new("."));
    let module_path = |name: &str| root.join(format!("{}.mod", name.replace('.', "/")));
    let at = |name: &str| {
        if name.is_empty() {
            path.to_path_buf()
        } else {
            module_path(name)
        }
    };
    let source = std::fs::read_to_string(path).map_err(|error| {
        (
            path.to_path_buf(),
            Diagnostic::new(syntax::Span::new(1, 1, 1), error.to_string()),
        )
    })?;
    modules::load(&source, &mut |name| {
        std::fs::read_to_string(module_path(name)).map_err(|error| error.to_string())
    })
    .map_err(|(name, error)| (at(&name), error))
}

/// Type-checks a parsed module and lowers it to HIR.
pub fn compile_module(mut module: syntax::Module, module_name: &str) -> Result<String, Diagnostic> {
    let prelude = parse(lex(include_str!("prelude.mod"))?)?;
    module.enums.extend(prelude.enums);
    // A module's own function of a prelude name is the one it calls.
    let own: std::collections::BTreeSet<String> = module.functions.iter().map(|one| one.name.clone()).collect();
    module.functions.extend(prelude.functions.into_iter().filter(|one| !own.contains(&one.name)));
    module.library = library::functions()?;
    module.library.extend(library::derived(&module)?);
    library::entry(&mut module)?;
    desugar::desugar(&mut module)?;
    semantic::compile(&module, module_name)
}
