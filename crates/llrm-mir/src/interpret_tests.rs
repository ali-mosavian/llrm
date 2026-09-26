use crate::datalayout::DataLayout;
use crate::interpret::{Trap, Val, run};
use crate::parse;
use crate::types::{Type, Types};

const LAYOUT: &str = "e-p:16:16-p1:32:16:16:16-i32:16-i64:16";

fn result(text: &str) -> Result<Val, Trap> {
    let module = parse::module(&format!("target datalayout = \"{LAYOUT}\"\n{text}")).unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(crate::verify::verify(&module), Vec::<String>::new());
    run(&module, "f", Vec::new(), 10_000)
}

fn int(bits: u128, width: u32) -> Result<Val, Trap> {
    Ok(Val::Int { bits, width })
}

#[test]
fn the_datalayout_places_fields_and_far_pointers_as_stated() {
    let layout = DataLayout::parse(LAYOUT).expect("parses");
    let mut types = Types::default();
    let (i16, i32) = (types.int(16), types.int(32));
    let pair = types.intern(Type::Struct { fields: vec![i16, i32], packed: false });
    assert_eq!(layout.struct_layout(&types, pair), (6, vec![0, 2]), "i32 aligned to 2 bytes");
    let far = layout.pointer(1);
    assert_eq!((far.bits, far.align, far.index_bits), (32, 2, 16));
    let i64 = types.int(64);
    assert_eq!(DataLayout::default().align(&types, i64), 4, "LLVM's default i64:32");
}

#[test]
fn flags_make_poison_where_the_value_does_not_fit() {
    assert_eq!(result("define i16 @f() {\n  %x = add nsw i16 32767, 1\n  ret i16 %x\n}\n"), Ok(Val::Poison));
    assert_eq!(result("define i16 @f() {\n  %x = add i16 32767, 1\n  ret i16 %x\n}\n"), int(0x8000, 16));
    assert_eq!(result("define i16 @f() {\n  %x = shl i16 1, 16\n  ret i16 %x\n}\n"), Ok(Val::Poison));
}

#[test]
fn undefined_behaviour_is_reported_not_run() {
    assert_eq!(result("define i16 @f() {\n  %x = sdiv i16 1, 0\n  ret i16 %x\n}\n"), Err(Trap::Undefined("a division by zero".to_owned())));
    let text = "define i16 @f() {\nentry:\n  %c = icmp eq i16 poison, 0\n  br i1 %c, label %a, label %b\na:\n  ret i16 1\nb:\n  ret i16 2\n}\n";
    assert_eq!(result(text), Err(Trap::Undefined("a branch on poison".to_owned())));
}

#[test]
fn a_far_offset_wraps_at_its_index_width() {
    let text = "@a = global [2 x i16] [i16 1, i16 2]\ndefine i32 @f() {\n  %far = addrspacecast ptr @a to ptr addrspace(1)\n  %p = getelementptr i16, ptr addrspace(1) %far, i16 32768\n  %base = ptrtoint ptr addrspace(1) %far to i32\n  %moved = ptrtoint ptr addrspace(1) %p to i32\n  %d = sub i32 %moved, %base\n  ret i32 %d\n}\n";
    // 32768 * 2 bytes is 0x10000: the 16-bit offset wraps to where it began.
    assert_eq!(result(text), int(0, 32));
}

#[test]
fn every_fixture_that_states_its_answer_gives_it() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    let mut ran = 0;
    for dir in std::fs::read_dir(&root).expect("tests/").flatten() {
        for file in std::fs::read_dir(dir.path()).into_iter().flatten().flatten() {
            let text = std::fs::read_to_string(file.path()).unwrap_or_default();
            let Some(want) = text.lines().find_map(|line| line.strip_prefix("; expect: ")).and_then(|one| one.parse::<u128>().ok()) else { continue };
            let module = parse::module(&text).expect("parses");
            let got = run(&module, "main", Vec::new(), 1_000_000);
            assert!(matches!(got, Ok(Val::Int { bits, .. }) if bits & 0xff == want), "{}: {got:?}", file.path().display());
            ran += 1;
        }
    }
    assert!(ran >= 6, "{ran} fixtures ran");
}
