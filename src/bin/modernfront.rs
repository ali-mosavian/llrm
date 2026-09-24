use std::env;
use std::fs;
use std::path::Path;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut tokens_only = false;
    let mut syntax_only = false;
    let mut input = None;
    let mut declare = None;
    let mut arguments = env::args().skip(1);
    while let Some(argument) = arguments.next() {
        if argument == "--declare" {
            let Some(language) = arguments.next().as_deref().and_then(llrm::frontends::modern::declarations::Language::named)
            else {
                eprintln!("modernfront: --declare takes h, bi or inc");
                return ExitCode::from(2);
            };
            declare = Some(language);
        } else if argument == "--tokens" {
            tokens_only = true;
        } else if argument == "--syntax" {
            syntax_only = true;
        } else if argument.starts_with('-') {
            eprintln!("modernfront: unknown option {argument:?}");
            return ExitCode::from(2);
        } else {
            if input.is_some() {
                eprintln!("modernfront: expected one input file");
                return ExitCode::from(2);
            }
            input = Some(argument);
        }
    }
    if tokens_only && syntax_only {
        eprintln!("modernfront: --tokens and --syntax are mutually exclusive");
        return ExitCode::from(2);
    }
    let Some(input) = input else {
        eprintln!("usage: modernfront [--tokens|--syntax|--declare h|bi|inc] FILE");
        return ExitCode::from(2);
    };
    let source = match fs::read_to_string(&input) {
        Ok(source) => source,
        Err(error) => {
            eprintln!("modernfront: {input}: {error}");
            return ExitCode::FAILURE;
        }
    };
    if tokens_only || syntax_only {
        let text = if tokens_only {
            llrm::frontends::modern::tokens_text(&source)
        } else {
            llrm::frontends::modern::syntax_text(&source)
        };
        return match text {
            Ok(text) => {
                print!("{text}");
                ExitCode::SUCCESS
            }
            Err(error) => report(&input, error),
        };
    }
    if let Some(language) = declare {
        return match llrm::frontends::modern::declare_file(Path::new(&input), language) {
            Ok(text) => {
                print!("{text}");
                ExitCode::SUCCESS
            }
            Err((path, error)) => report(&path.display().to_string(), error),
        };
    }
    match llrm::frontends::modern::compile_file(Path::new(&input)) {
        Ok(hir) => {
            print!("{hir}");
            ExitCode::SUCCESS
        }
        Err((path, error)) => report(&path.display().to_string(), error),
    }
}

fn report(path: &str, error: llrm::frontends::modern::Diagnostic) -> ExitCode {
    eprintln!("{path}:{error}");
    ExitCode::FAILURE
}
