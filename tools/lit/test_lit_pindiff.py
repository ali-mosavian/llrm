import pindiff

PIN = """; ModuleID = 'x'
source_filename = "x"
define i32 @f(i32 %x) #0 {
entry:
  %scevgep = add i32 %x, 1
  br label %loop
loop:                                             ; preds = %loop, %entry
  %lsr.iv = phi i32 [ %scevgep, %entry ], [ %lsr.iv, %loop ]
  br label %loop
}
attributes #0 = { nounwind }
"""
OURS = """define i32 @f(i32 %x) nounwind {
entry:
  %0 = add i32 %x, 1
  br label %1
1:
  %2 = phi i32 [ %0, %entry ], [ %2, %1 ]
  br label %1
}
"""


def test_names_attribute_groups_and_comments_are_not_a_difference():
    assert pindiff.text_form(PIN) != pindiff.text_form(OURS)
    assert pindiff.shape_form(PIN) == pindiff.shape_form(OURS)


def test_a_different_instruction_is_a_difference():
    assert pindiff.shape_form(PIN) != pindiff.shape_form(OURS.replace("add i32", "sub i32"))


def test_a_different_operand_order_is_a_difference():
    assert pindiff.shape_form(PIN) != pindiff.shape_form(OURS.replace("[ %0, %entry ], [ %2, %1 ]", "[ %2, %1 ], [ %0, %entry ]"))
