use super::InlineVec;
use crate::module::BlockId;

#[test]
fn test_elements_read_in_the_order_pushed_inline_and_spilled() {
    for n in 0..9_u32 {
        let list: InlineVec<BlockId, 4> = (0..n).map(|k| BlockId(k * 3)).collect();
        let want: Vec<BlockId> = (0..n).map(|k| BlockId(k * 3)).collect();
        assert_eq!(list, want, "{n} elements");
        assert_eq!(list.len() as u32, n);
        assert_eq!(list.clone().into_iter().collect::<Vec<_>>(), want);
        assert_eq!(Vec::from(list), want);
    }
}

#[test]
fn test_pop_truncate_clear_insert_remove_and_retain_agree_with_a_vec() {
    let mut list: InlineVec<i64, 2> = [5, 6].into();
    let mut vec = vec![5_i64, 6];
    for step in 0..12_i64 {
        list.push(step * 7);
        vec.push(step * 7);
        if step % 3 == 0 {
            assert_eq!(list.pop(), vec.pop());
        }
        if step % 4 == 1 {
            list.insert(1, -step);
            vec.insert(1, -step);
        }
        if step % 5 == 2 {
            assert_eq!(list.remove(0), vec.remove(0));
        }
        assert_eq!(list, vec, "after step {step}");
    }
    list.retain(|one| one % 2 == 0);
    vec.retain(|one| one % 2 == 0);
    assert_eq!(list, vec);
    list.truncate(1);
    vec.truncate(1);
    assert_eq!(list, vec);
    list.clear();
    assert!(list.is_empty());
}

#[test]
fn test_a_list_of_up_to_n_is_not_on_the_heap_and_one_past_is() {
    let few: InlineVec<i64, 2> = [1, 2].into();
    assert!(matches!(few, InlineVec::Few(_, _)));
    let many: InlineVec<i64, 2> = [1, 2, 3].into();
    assert!(matches!(many, InlineVec::Many(_)));
    let mut grown = few.clone();
    grown.push(3);
    assert!(matches!(grown, InlineVec::Many(_)));
}

/// What the owner of one pays for it: a `Vec` is 24 bytes, so a payload of 16
/// bytes (and the length) keeps the struct as it was.
#[test]
fn test_the_size_of_the_common_instances() {
    let vec = std::mem::size_of::<Vec<i64>>();
    assert_eq!(vec, 24);
    println!(
        "InlineVec<i64,1> {} B, <i64,2> {} B, <u32,3> {} B, <u32,4> {} B",
        std::mem::size_of::<InlineVec<i64, 1>>(),
        std::mem::size_of::<InlineVec<i64, 2>>(),
        std::mem::size_of::<InlineVec<u32, 3>>(),
        std::mem::size_of::<InlineVec<u32, 4>>(),
    );
}

/// Two operands held inline made `Operands` 32 bytes, so every `Instruction`
/// 104 against 96; the metadata list behind a pointer pays for it.
#[test]
fn an_instruction_is_no_larger_than_96_bytes_with_two_operands_inline() {
    assert!(size_of::<crate::module::Instruction>() <= 96, "{} bytes", size_of::<crate::module::Instruction>());
}
