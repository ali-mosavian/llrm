//! `llrm-opt [-S] [-passes=P | -PASS] [-mtriple=T] [-o FILE] [FILE|-]`: the
//! shape of LLVM's `opt` over the passes llrm-transforms has, so the pin's lit
//! tests can run through it. Exit 77, with `unsupported: X` on stderr, for a
//! pass or flag it does not have; 78 for textual IR its parser does not read.

use std::io::{Read, Write};
use std::process::ExitCode;
use std::rc::Rc;

use llrm_mir::passes::PassManager;
use llrm_mir::target::{Machine, Neutral};
use llrm_mir::tti::Tti;

const UNSUPPORTED: u8 = 77;
const UNREAD: u8 = 78;

/// `Neutral` with the cost model of the triple's target.
struct Opt(Option<Rc<dyn Tti>>);

macro_rules! neutral {
    ($($name:ident($($arg:ident: $ty:ty),*) -> $ret:ty;)*) => {
        $(fn $name(&self, $($arg: $ty),*) -> $ret { Neutral.$name($($arg),*) })*
    };
}

impl Machine for Opt {
    neutral! {
        spaces() -> llrm_mir::spaces::Spaces;
        foreign_span(selectors: (i64, i64), offsets: (i64, i64), width: i64) -> Option<(i64, i64)>;
        costs() -> llrm_mir::target::OperationCosts;
        registers() -> i64;
        call_registers() -> i64;
        address_forms() -> Vec<llrm_mir::target::AddressForm>;
    }

    fn tti(&self) -> Option<Rc<dyn Tti>> {
        self.0.clone()
    }
}

/// The cost model of `triple`'s architecture; none for the ones without.
fn tti_of(triple: &str) -> Option<Rc<dyn Tti>> {
    let arch = triple.split('-').next().unwrap_or("");
    let cpu = match arch {
        "x86_64" => "x86-64",
        "i386" => "i386",
        "i486" => "i486",
        "i586" => "pentium",
        "i686" => "pentiumpro",
        "aarch64" | "arm64" => {
            let machine = llrm_arm64::machine::Arm64Machine { platform: llrm_arm64::machine::Platform::Linux };
            return machine.tti();
        }
        _ => return None,
    };
    llrm_x86::tti::X86::of_cpu(cpu).map(|tti| Rc::new(tti) as Rc<dyn Tti>)
}

/// The `target triple = "..."` the module states.
fn triple_of(text: &str) -> Option<&str> {
    let rest = text.lines().find_map(|line| line.trim_start().strip_prefix("target triple"))?;
    rest.split('"').nth(1)
}

fn unsupported(what: &str) -> ExitCode {
    eprintln!("unsupported: {what}");
    ExitCode::from(UNSUPPORTED)
}

fn main() -> ExitCode {
    let (mut passes, mut triple, mut output, mut input) = (String::new(), None, None, None);
    let mut quiet = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let flag = arg.trim_start_matches('-');
        match (arg.starts_with('-') && arg != "-", flag) {
            (true, "S" | "verify-each" | "opaque-pointers" | "enable-new-pm") => {}
            (true, "disable-output") => quiet = true,
            (true, "o") => output = args.next(),
            (true, "mtriple") => triple = args.next(),
            (true, _) if flag.starts_with("passes=") => {
                if !passes.is_empty() {
                    passes.push(',');
                }
                passes.push_str(&flag["passes=".len()..]);
            }
            (true, _) if flag.starts_with("mtriple=") => triple = Some(flag["mtriple=".len()..].to_owned()),
            (true, _) if flag.starts_with("o=") => output = Some(flag[2..].to_owned()),
            (true, _) => match llrm_transforms::pipeline_text::legacy(flag) {
                Some(pass) => {
                    if !passes.is_empty() {
                        passes.push(',');
                    }
                    passes.push_str(pass);
                }
                None => return unsupported(&arg),
            },
            (false, _) => input = Some(arg),
        }
    }
    let elements = match llrm_transforms::pipeline_text::parse(&passes) {
        Ok(elements) => elements,
        Err(error) => {
            eprintln!("llrm-opt: -passes: {error}");
            return ExitCode::FAILURE;
        }
    };
    let mut manager = PassManager::default();
    manager.verify_each = true;
    if let Err(llrm_transforms::pipeline_text::Unsupported(what)) =
        llrm_transforms::pipeline_text::build(&elements, &mut manager)
    {
        return unsupported(&what);
    }
    let mut text = String::new();
    let read = match input.as_deref() {
        None | Some("-") => std::io::stdin().read_to_string(&mut text).map(drop),
        Some(path) => std::fs::read_to_string(path).map(|all| text = all),
    };
    if let Err(error) = read {
        eprintln!("llrm-opt: {error}");
        return ExitCode::FAILURE;
    }
    // The subset parser has no `target triple`; `triple_of` read it above.
    let stripped: String = text
        .lines()
        .filter(|line| !line.trim_start().starts_with("target triple"))
        .map(|line| format!("{line}\n"))
        .collect();
    let mut module = match llrm_mir::parse::module(&stripped) {
        Ok(module) => module,
        Err(error) => {
            eprintln!("unsupported: input: {error}");
            return ExitCode::from(UNREAD);
        }
    };
    let target = Rc::new(Opt(triple.as_deref().or_else(|| triple_of(&text)).and_then(tti_of)));
    if let Err(error) = manager.run_module(&mut module, target) {
        // A datalayout spec the layout parser lacks is input it cannot read.
        let unread = error.contains("in the datalayout");
        eprintln!("{}llrm-opt: {error}", if unread { "unsupported: input: " } else { "" });
        return if unread { ExitCode::from(UNREAD) } else { ExitCode::FAILURE };
    }
    if !quiet {
        let text = llrm_mir::print::module(&module);
        let written = match output.as_deref() {
            None | Some("-") => std::io::stdout().write_all(text.as_bytes()),
            Some(path) => std::fs::write(path, text),
        };
        if let Err(error) = written {
            eprintln!("llrm-opt: {error}");
            return ExitCode::FAILURE;
        }
    }
    ExitCode::SUCCESS
}
