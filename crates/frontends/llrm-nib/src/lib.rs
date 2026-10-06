//! Frontend for Nib, a small systems language for real-mode DOS.
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
pub mod lsp;
pub mod cli;
pub mod nibstages;
pub mod modules;
pub mod parser;
pub mod resumable;
pub mod scopes;
pub mod standard;
pub mod semantic;
pub mod syntax;

#[cfg(test)]
mod test_borrowck;
#[cfg(test)]
mod test_language;
#[cfg(test)]
pub(crate) mod test_nib_frontend;

pub use error::Diagnostic;
pub use lexer::lex;
pub use parser::parse;

/// What a build asks of the frontend beyond the source.
#[derive(Clone, Debug)]
pub struct Frontend {
    /// The data layout and address spaces of the target the program is for: how many bytes a
    /// near and a far pointer are, which the language's `*near` and `*far` mean there.
    pub layout: llrm_target::layout::Layout,
    /// The bytes an argument takes on the stack at least: the target's stack slot.
    pub slot: u32,
    /// The bits of the target's code (its `object.toml`): what inline assembly is assembled for.
    pub bits: u32,
    /// The calling conventions the target defines, the first its programs' own.
    pub conventions: Vec<String>,
    /// The target's OS layer under Nib's runtime.
    pub os: Os,
    /// Index and slice bounds go unchecked, as in `unsafe`: `--unchecked-bounds`.
    pub unchecked_bounds: bool,
    /// `-g`: source lines and debug information.
    pub debug: bool,
    /// Each function compares SP with the runtime's limit on entry: `-fsanitize=stack`.
    pub checked_stack: bool,
    /// Warn where `far` or `huge` is written for a target where far is near (`-Wno-distance` turns
    /// it off, for the runtime, which writes `*far` for the targets that have one).
    pub warn_distance: bool,
    /// What the last compile warned of, for the caller to print.
    pub warnings: std::rc::Rc<std::cell::RefCell<Vec<Diagnostic>>>,
}

impl Default for Frontend {
    /// For real mode, where the language began: a caller that knows its target sets `layout`.
    fn default() -> Self {
        Self { layout: llrm_x86_code16::layout(), slot: 2, bits: 16, conventions: llrm_target::Target::conventions(&llrm_x86_code16::Code16).iter().map(|one| (*one).to_owned()).collect(), os: Os::of(llrm_target::Target::runtime(&llrm_x86_code16::Code16, "nib").expect("real mode has a Nib runtime")).expect("its description reads"), unchecked_bounds: false, debug: false, checked_stack: false, warn_distance: true, warnings: Default::default() }
    }
}

/// What a target's OS layer says of Nib's runtime (`runtime/nib/nib.toml` of its crate).
#[derive(Clone, Debug)]
pub struct Os {
    /// `std.os` and `os`: the module the runtime's routines call the operating system through.
    pub module: String,
    /// What `-fsanitize=stack` compares and calls.
    pub stack: llrm_core::hir::model::StackCheck,
    /// The stack the start-up reserves; the object's own adds to it.
    pub stack_base: i64,
    /// Whether the start-up zeroes the far uninitialised data.
    pub far_bss: bool,
    /// The directory the description is in, and the assembly files it names there.
    pub directory: String,
    pub start: String,
    pub dos: String,
}

impl Os {
    pub fn of(description: llrm_target::runtime::Description) -> Result<Self, String> {
        let table = description.table()?;
        let text = |key: &str| description.string(key);
        let file = |key: &str| -> Result<String, String> { let name = text(key)?; description.file(&name).map(str::to_owned).ok_or(format!("the runtime description names {name}, which is not shipped")) };
        let stack = toml::Value::Table(file("stack")?.parse().map_err(|error: toml::de::Error| error.to_string())?);
        Ok(Self {
            module: file("os")?,
            stack: llrm_core::hir::model::StackCheck::from_toml(&stack)?,
            stack_base: table.get("stack_base").and_then(|one| one.as_integer()).ok_or("stack_base is not an integer")?,
            far_bss: table.get("far_bss").and_then(|one| one.as_bool()).ok_or("far_bss is not a boolean")?,
            directory: description.directory.to_owned(),
            start: text("start")?,
            dos: text("dos")?,
        })
    }
}

/// The bytes of the two pointers the language has.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Sizes {
    pub near: u32,
    pub far: u32,
    /// Whether the near and the far pointer reach different spaces: a near one reaches only the
    /// data group.
    pub segmented: bool,
    /// A stack slot's bytes.
    pub slot: u32,
}

impl Frontend {
    /// The convention the language's own functions have: the target's first.
    pub fn native(&self) -> syntax::Abi {
        self.conventions.first().and_then(|name| syntax::Abi::named(name)).expect("a target defines a calling convention the language names")
    }

    /// The pointers' sizes: what the datalayout says of the near and the far space.
    pub fn sizes(&self) -> Sizes {
        let layout = llrm_mir::datalayout::DataLayout::parse(&self.layout.datalayout).expect("a target's datalayout parses");
        let bytes = |space: u32| layout.pointer(space).bits / 8;
        Sizes { near: bytes(self.layout.spaces.near), far: bytes(self.layout.spaces.far), segmented: !self.layout.spaces.far_is_near(), slot: self.slot }
    }
}

pub fn compile(source: &str, module_name: &str) -> Result<String, Diagnostic> {
    let tokens = lex(source)?;
    compile_module(parse(tokens)?, module_name, &Frontend::default())
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
/// files under the same directory, `a.b` at `a/b.nib`.
pub fn compile_file(path: &std::path::Path, frontend: &Frontend) -> Result<String, (std::path::PathBuf, Diagnostic)> {
    let module = load_file(path, &frontend.os, frontend.sizes().near)?;
    let sources = module.sources.clone();
    compile_module(module, module_name(path), frontend).map_err(|error| located(path, &sources, error))
}

/// The `.H`, `.BI` or `.INC` declarations of the program at `path`'s exports.
pub fn declare_file(
    path: &std::path::Path,
    language: declarations::Language,
    frontend: &Frontend,
) -> Result<String, (std::path::PathBuf, Diagnostic)> {
    let module = load_file(path, &frontend.os, frontend.sizes().near)?;
    declarations::declarations_on(&module, module_name(path), language, frontend.sizes().segmented, frontend.slot).map_err(|error| located(path, &module.sources, error))
}

fn module_name(path: &std::path::Path) -> &str {
    path.file_stem().and_then(|one| one.to_str()).unwrap_or("module")
}

/// The module at `path`, linked with every module it imports.
/// `error` with the file of the module its span is in.
fn located(path: &std::path::Path, sources: &[String], error: Diagnostic) -> (std::path::PathBuf, Diagnostic) {
    let (name, error) = in_module(sources, error);
    (module_path(path, &name), error)
}

/// `error` with the name of the module its span is in.
fn in_module(sources: &[String], error: Diagnostic) -> modules::Located {
    (sources.get(usize::from(error.span.module)).cloned().unwrap_or_default(), error)
}

/// Where the module `name`, which the program at `path` imports, is read
/// from: `<std.io>` for one the compiler supplies.
pub fn module_path(path: &std::path::Path, name: &str) -> std::path::PathBuf {
    if name.is_empty() {
        path.to_path_buf()
    } else if standard::supplied(name) {
        std::path::PathBuf::from(format!("<{name}>"))
    } else {
        modules::file(path.parent().unwrap_or(std::path::Path::new(".")), name)
    }
}

fn load_file(path: &std::path::Path, os: &Os, near_bytes: u32) -> Result<syntax::Module, (std::path::PathBuf, Diagnostic)> {
    let source = std::fs::read_to_string(path).map_err(|error| {
        (
            path.to_path_buf(),
            Diagnostic::new(syntax::Span::new(1, 1, 1), error.to_string()),
        )
    })?;
    modules::load_for(&source, &mut |name| {
        if matches!(name, "os" | "std.os") {
            return Ok(os.module.clone());
        }
        std::fs::read_to_string(module_path(path, name)).map_err(|error| error.to_string())
    }, near_bytes)
    .map_err(|(name, error)| (module_path(path, &name), error))
}

/// Type-checks a parsed module and lowers it to HIR.
pub fn compile_module(module: syntax::Module, module_name: &str, frontend: &Frontend) -> Result<String, Diagnostic> {
    semantic::compile(&prepared(module)?, module_name, frontend)
}

/// The program whose main module is `source`, `read` giving each module it
/// imports, type-checked as `compile_file` would compile it.
pub struct Checked {
    /// Each module as written, when all of them parse.
    pub loaded: Option<modules::Loaded>,
    /// What the checker learned of the names the program spells.
    pub facts: Vec<semantic::Fact>,
    /// The first error, and the module it is in.
    pub error: Option<modules::Located>,
}

pub fn check(source: &str, read: &mut dyn FnMut(&str) -> Result<String, String>) -> Checked {
    // An editor checks for the language's first target: `std.os` is its OS layer.
    let mut read = |name: &str| if name == "std.os" { Ok(Frontend::default().os.module) } else { read(name) };
    let loaded = match modules::read_all(source, &mut read) {
        Ok(loaded) => loaded,
        Err(error) => return Checked { loaded: None, facts: Vec::new(), error: Some(error) },
    };
    let sources = loaded.sources.clone();
    let prepared = loaded.clone().linked().and_then(|module| prepared(module).map_err(|error| in_module(&sources, error)));
    let (facts, error) = match prepared {
        Ok(module) => {
            let (facts, checked) = semantic::check(&module);
            (facts, checked.err().map(|error| in_module(&sources, error)))
        }
        Err(error) => (Vec::new(), Some(error)),
    };
    Checked { loaded: Some(loaded), facts, error }
}

/// A linked module with the prelude and library it is checked with, desugared.
fn prepared(mut module: syntax::Module) -> Result<syntax::Module, Diagnostic> {
    let prelude = parse(lex(include_str!("prelude.nib"))?)?;
    module.enums.extend(prelude.enums);
    // A module's own function or protocol of a prelude name is the one it names.
    let own: std::collections::BTreeSet<String> = module.functions.iter().map(|one| one.name.clone()).collect();
    module.functions.extend(prelude.functions.into_iter().filter(|one| !own.contains(&one.name)));
    let own: std::collections::BTreeSet<String> = module.protocols.iter().map(|one| one.name.clone()).collect();
    module.protocols.extend(prelude.protocols.into_iter().filter(|one| !own.contains(&one.name)));
    module.library = library::functions()?;
    module.library.extend(library::derived(&module)?);
    library::entry(&mut module)?;
    desugar::desugar(&mut module)?;
    Ok(module)
}

#[cfg(test)]
mod test_execute;
#[cfg(test)]
mod test_pipeline;
#[cfg(test)]
mod test_debug;
