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
            let Some(language) = arguments.next().as_deref().and_then(llrm_nib::declarations::Language::named)
            else {
                eprintln!("nibfront: --declare takes h, bi or inc");
                return ExitCode::from(2);
            };
            declare = Some(language);
        } else if argument == "--tokens" {
            tokens_only = true;
        } else if argument == "--syntax" {
            syntax_only = true;
        } else if argument.starts_with('-') {
            eprintln!("nibfront: unknown option {argument:?}");
            return ExitCode::from(2);
        } else {
            if input.is_some() {
                eprintln!("nibfront: expected one input file");
                return ExitCode::from(2);
            }
            input = Some(argument);
        }
    }
    if tokens_only && syntax_only {
        eprintln!("nibfront: --tokens and --syntax are mutually exclusive");
        return ExitCode::from(2);
    }
    let Some(input) = input else {
        eprintln!("usage: nibfront [--tokens|--syntax|--declare h|bi|inc] FILE");
        return ExitCode::from(2);
    };
    let source = match fs::read_to_string(&input) {
        Ok(source) => source,
        Err(error) => {
            eprintln!("nibfront: {input}: {error}");
            return ExitCode::FAILURE;
        }
    };
    if tokens_only || syntax_only {
        let text = if tokens_only {
            llrm_nib::tokens_text(&source)
        } else {
            llrm_nib::syntax_text(&source)
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
        return match llrm_nib::declare_file(Path::new(&input), language) {
            Ok(text) => {
                print!("{text}");
                ExitCode::SUCCESS
            }
            Err((path, error)) => report(&path.display().to_string(), error),
        };
    }
    match llrm_nib::compile_file(Path::new(&input), &Default::default()) {
        Ok(hir) => {
            print!("{hir}");
            ExitCode::SUCCESS
        }
        Err((path, error)) => report(&path.display().to_string(), error),
    }
}

fn report(path: &str, error: llrm_nib::Diagnostic) -> ExitCode {
    eprintln!("{path}:{error}");
    ExitCode::FAILURE
}
