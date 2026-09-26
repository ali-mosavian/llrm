use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use qbfront::generated_parser::parse_vertical_slice;
use qbfront::{source, Dialect, Module};

#[derive(Clone, Debug)]
struct CompatCase {
    id: String,
    dialect: Dialect,
    source: PathBuf,
    source_id: String,
}

fn cases(root: &Path) -> Vec<CompatCase> {
    let mut out = Vec::new();
    for (profile, dialect) in [
        ("qb45", Dialect::QuickBasic45),
        ("pds71", Dialect::Pds71),
        ("vbdos", Dialect::VbDos),
    ] {
        let directory = root.join(profile);
        let suite = fs::read_to_string(directory.join("suite.toml")).unwrap();
        let mut name = None;
        for line in suite.lines() {
            if let Some(value) = line.trim().strip_prefix("name = ") {
                name = Some(value.trim().trim_matches('"').to_string());
                continue;
            }
            let Some(value) = line.trim().strip_prefix("source = ") else {
                continue;
            };
            let name = name
                .take()
                .expect("every compat source must have a preceding case name");
            let source_id = format!("{profile}/{}", value.trim().trim_matches('"'));
            out.push(CompatCase {
                id: format!("{profile}/{name}"),
                dialect,
                source: directory.join(value.trim().trim_matches('"')),
                source_id,
            });
        }
    }
    out
}

fn golden_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/legacy_ast_goldens")
}

fn golden_path(case: &CompatCase) -> PathBuf {
    let (profile, name) = case.id.split_once('/').expect("case id has profile");
    golden_root().join(format!("{profile}--{name}.ast"))
}

fn golden_text(case: &CompatCase, module: &Module) -> String {
    format!(
        "# qbfront legacy-derived AST snapshot; schema migrations require projection proof.\n# case-id: {}\n# dialect: {:?}\n# source: {}\n\n{module:#?}\n",
        case.id, case.dialect, case.source_id
    )
}

fn golden_cases(root: &Path) -> Vec<CompatCase> {
    let all = cases(root);
    let by_id = all
        .iter()
        .map(|case| (case.id.as_str(), case))
        .collect::<BTreeMap<_, _>>();
    let index = fs::read_to_string(golden_root().join("accepted-cases.tsv"))
        .expect("checked-in legacy AST golden index");
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for (line_number, line) in index.lines().enumerate() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let fields = line.split('\t').collect::<Vec<_>>();
        assert_eq!(
            fields.len(),
            3,
            "golden index line {} must have case id, dialect, and source",
            line_number + 1
        );
        let case = by_id.get(fields[0]).unwrap_or_else(|| {
            panic!(
                "golden case {:?} is absent from compat manifests",
                fields[0]
            )
        });
        assert_eq!(format!("{:?}", case.dialect), fields[1]);
        assert_eq!(case.source_id, fields[2]);
        assert!(
            seen.insert(case.id.clone()),
            "duplicate golden case id {}",
            case.id
        );
        out.push((*case).clone());
    }
    assert_eq!(
        out.len(),
        79,
        "golden index is the legacy-accepted corpus gate"
    );
    out
}

fn assert_matches_golden(case: &CompatCase) {
    let source =
        source::load(&case.source, &[case.source.parent().unwrap().to_path_buf()]).unwrap();
    let generated = parse_vertical_slice(&source, case.dialect)
        .unwrap_or_else(|error| panic!("generated parser rejected golden {}: {error:?}", case.id));
    let golden = fs::read_to_string(golden_path(case)).unwrap();
    assert_eq!(
        golden_text(case, &generated.module),
        golden,
        "generated AST differs from legacy golden for {}",
        case.id
    );
}

#[test]
fn checked_in_legacy_ast_goldens_cover_the_accepted_compat_cases() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("compat");
    let cases = golden_cases(&root);
    for case in cases {
        let golden = golden_path(&case);
        assert!(golden.is_file(), "missing AST golden {}", golden.display());
        let text = fs::read_to_string(&golden).unwrap();
        assert!(
            text.starts_with(&format!("# qbfront legacy-derived AST snapshot; schema migrations require projection proof.\n# case-id: {}\n# dialect: {:?}\n# source: {}\n\n", case.id, case.dialect, case.source_id)),
            "golden {} has stale metadata", golden.display()
        );
    }
}

#[test]
fn generated_parser_matches_checked_in_legacy_ast_goldens_when_accepted() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("compat");
    let mut compared = 0;
    for case in golden_cases(&root) {
        let source =
            source::load(&case.source, &[case.source.parent().unwrap().to_path_buf()]).unwrap();
        if parse_vertical_slice(&source, case.dialect).is_err() {
            continue;
        }
        compared += 1;
        assert_matches_golden(&case);
    }
    assert_eq!(
        compared, 79,
        "the generated parser must match every legacy-accepted manifest identity"
    );
}

#[test]
fn generated_parser_matches_all_legacy_ast_goldens_before_the_switch() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("compat");
    let cases = golden_cases(&root);
    for case in &cases {
        assert_matches_golden(case);
    }
    assert_eq!(
        cases.len(),
        79,
        "the production-parser switch must compare every legacy-accepted manifest identity"
    );
}

#[test]
fn generated_parser_accepts_the_vbdos_source_superset_for_every_profile() {
    for dialect in [
        Dialect::QBasic11,
        Dialect::QuickBasic45,
        Dialect::Pds71,
        Dialect::VbDos,
    ] {
        assert!(parse_vertical_slice("dim pos_x as long", dialect).is_ok());
        assert!(parse_vertical_slice("option explicit", dialect).is_ok());
        assert!(parse_vertical_slice("on local error goto handler", dialect).is_ok());
        assert!(parse_vertical_slice(
            "declare function compat_external cdecl alias \"compat_external\" (byval source_value as long) as long",
            dialect,
        )
        .is_ok());
        assert!(parse_vertical_slice(
            "dim continued_value as long\ncontinued_value = 1 + _\n 2\n",
            dialect,
        )
        .is_ok());
    }
}
