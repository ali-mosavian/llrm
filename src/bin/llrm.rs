use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use llrm::driver::{self, QbOptions};
use llrm::frontend::qb::{Dialect, source};
use llrm::hir::RuntimeProfile;

fn main() -> ExitCode {
    match Invocation::parse(env::args().skip(1)) {
        Ok(invocation) => invocation.run(),
        Err(message) => {
            eprintln!("llrm: {message}");
            ExitCode::from(2)
        }
    }
}

struct Invocation {
    input: PathBuf,
    output: Option<PathBuf>,
    input_kind: InputKind,
    include_dirs: Vec<PathBuf>,
    qb: QbOptions,
}

#[derive(Clone, Copy)]
enum InputKind {
    Qb,
    Wcc,
    Omf,
}

impl Invocation {
    fn parse(arguments: impl Iterator<Item = String>) -> Result<Self, String> {
        let mut arguments = arguments.peekable();
        let mut input = None;
        let mut output = None;
        let mut input_kind = None;
        let mut include_dirs = Vec::new();
        let mut qb = QbOptions::default();

        while let Some(argument) = arguments.next() {
            match argument.as_str() {
                "-x" => input_kind = Some(parse_input_kind(required(&mut arguments, "-x")?)?),
                "-o" => output = Some(PathBuf::from(required(&mut arguments, "-o")?)),
                "--include" => {
                    include_dirs.push(PathBuf::from(required(&mut arguments, "--include")?));
                }
                "--dialect" => {
                    let value = required(&mut arguments, "--dialect")?;
                    qb.dialect = Dialect::parse(&value)
                        .ok_or_else(|| format!("unknown QB dialect {value:?}"))?;
                }
                "--runtime" => {
                    let value = required(&mut arguments, "--runtime")?;
                    qb.runtime = match value.as_str() {
                        "qb45" => RuntimeProfile::Qb45,
                        "pds71" => RuntimeProfile::Pds71,
                        "vbdos" => RuntimeProfile::Vbdos,
                        _ => return Err(format!("unknown QB runtime {value:?}")),
                    };
                }
                "--array-order" => {
                    let value = required(&mut arguments, "--array-order")?;
                    qb.row_major = match value.as_str() {
                        "column-major" => false,
                        "row-major" => true,
                        _ => return Err(format!("unknown array order {value:?}")),
                    };
                }
                "--huge-arrays" => qb.huge_arrays = true,
                "--checked-arrays" => qb.checked_arrays = true,
                "--mbf" => qb.mbf = true,
                "--alternate-math" => qb.alternate_math = true,
                "-h" | "--help" => return Err(usage().to_owned()),
                _ if argument.starts_with('-') => {
                    return Err(format!("unknown option {argument:?}\n{}", usage()));
                }
                _ => {
                    if input.is_some() {
                        return Err("expected exactly one input path".to_owned());
                    }
                    input = Some(PathBuf::from(argument));
                }
            }
        }

        let input = input.ok_or_else(|| usage().to_owned())?;
        let input_kind = input_kind
            .or_else(|| infer_input_kind(&input))
            .ok_or_else(|| "cannot infer input kind; use -x qb, -x wcc, or -x omf".to_owned())?;
        Ok(Self {
            input,
            output,
            input_kind,
            include_dirs,
            qb,
        })
    }

    fn run(self) -> ExitCode {
        match self.input_kind {
            InputKind::Qb => self.run_qb(),
            InputKind::Wcc => unsupported("the WCC capture frontend has not been ported yet"),
            InputKind::Omf => unsupported("the OMF rewrite frontend has not been ported yet"),
        }
    }

    fn run_qb(self) -> ExitCode {
        let loaded = match source::load_with_map(&self.input, &self.include_dirs) {
            Ok(source) => source,
            Err(error) => return failure(format!("{}: {error}", self.input.display())),
        };
        let module_name = self
            .input
            .file_stem()
            .and_then(|name| name.to_str())
            .unwrap_or("module");
        let program = match driver::compile_qb(&loaded.text, module_name, self.qb) {
            Ok(program) => program,
            Err(driver::Error::Parse(error)) => {
                let location = loaded.location(error.span.line);
                let path = location.map_or(self.input.as_path(), |one| one.path.as_path());
                let line = location.map_or(error.span.line, |one| one.line);
                return failure(format!(
                    "{}:{}:{}: {}",
                    path.display(),
                    line,
                    error.span.start + 1,
                    error.message
                ));
            }
            Err(error) => return failure(format!("{}: {error}", self.input.display())),
        };
        let text = llrm::hir::write_text(&program);
        match self.output {
            Some(path) => match fs::write(&path, text) {
                Ok(()) => ExitCode::SUCCESS,
                Err(error) => failure(format!("{}: {error}", path.display())),
            },
            None => match io::stdout().write_all(text.as_bytes()) {
                Ok(()) => ExitCode::SUCCESS,
                Err(error) => failure(format!("stdout: {error}")),
            },
        }
    }
}

fn required(arguments: &mut impl Iterator<Item = String>, option: &str) -> Result<String, String> {
    arguments
        .next()
        .ok_or_else(|| format!("{option} requires a value"))
}

fn parse_input_kind(value: String) -> Result<InputKind, String> {
    match value.as_str() {
        "qb" => Ok(InputKind::Qb),
        "wcc" => Ok(InputKind::Wcc),
        "omf" => Ok(InputKind::Omf),
        _ => Err(format!("unknown input kind {value:?}")),
    }
}

fn infer_input_kind(path: &Path) -> Option<InputKind> {
    match path.extension()?.to_str()?.to_ascii_lowercase().as_str() {
        "bas" => Some(InputKind::Qb),
        "cgs" => Some(InputKind::Wcc),
        "obj" | "lib" => Some(InputKind::Omf),
        _ => None,
    }
}

fn unsupported(message: &str) -> ExitCode {
    failure(message.to_owned())
}

fn failure(message: String) -> ExitCode {
    eprintln!("llrm: {message}");
    ExitCode::FAILURE
}

fn usage() -> &'static str {
    "usage: llrm [-x qb|wcc|omf] [-o FILE] [QB OPTIONS] INPUT"
}
