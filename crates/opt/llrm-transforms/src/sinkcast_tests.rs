//! A cast is made again in the blocks that read it.

use crate::cgpaddr::CgpAddr;
use crate::testing::{parsed, printed};

const HEAD: &str = "target datalayout = \"e-p:16:16-i32:16-i64:16-f80:16-n8:16:32\"\n\n";

fn after(text: &str) -> String {
    let mut module = parsed(&format!("{HEAD}{text}"));
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.add(CgpAddr);
    manager.run_module(&mut module, std::rc::Rc::new(llrm_x86_m16::Dos::default())).unwrap();
    printed(&module)
}

/// A pointer made an integer in one block and stored in the next: the cast is
/// made in the block that reads it.
#[test]
fn test_a_pointer_cast_read_in_another_block_is_made_there() {
    let text = after(
        "define void @f(ptr %a, ptr %q) {
b0:
  %p = ptrtoint ptr %a to i16
  br label %b1

b1:
  store i16 %p, ptr %q
  ret void
}
",
    );
    let (entry, rest) = text.split_once("b1:").expect("a second block");
    assert!(!entry.contains("ptrtoint"), "{text}");
    assert!(rest.contains("ptrtoint ptr %a to i16"), "{text}");
}

/// A widening to a type no register holds is made again where it is read.
#[test]
fn test_a_zext_to_i64_read_in_another_block_is_made_there() {
    let text = after(
        "define i64 @f(i16 %a) {
b0:
  %w = zext i16 %a to i64
  br label %b1

b1:
  %r = add i64 %w, 1
  ret i64 %r
}
",
    );
    let (entry, rest) = text.split_once("b1:").expect("a second block");
    assert!(!entry.contains("zext"), "{text}");
    assert!(rest.contains("zext i16 %a to i64"), "{text}");
}

/// A widening to a register is left where it is.
#[test]
fn test_a_zext_to_a_register_type_stays() {
    let source = "define i32 @f(i16 %a) {
b0:
  %w = zext i16 %a to i32
  br label %b1

b1:
  %r = add i32 %w, 1
  ret i32 %r
}
";
    assert_eq!(after(source), printed(&parsed(&format!("{HEAD}{source}"))));
}
