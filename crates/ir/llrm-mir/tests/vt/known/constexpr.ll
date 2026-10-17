@g = global i32 0
@g2 = global i64 0, align 8
@g3 = global [20 x i8] zeroinitializer

define i1 @const_sdiv_i1() {
  %div = sdiv i1 ptrtoint (ptr @g to i1), ptrtoint (ptr @g2 to i1)
  ret i1 %div
}

define i1 @const_udiv_i1() {
  %div = udiv i1 ptrtoint (ptr @g to i1), ptrtoint (ptr @g2 to i1)
  ret i1 %div
}

define i64 @const_and() {
  %a = and i64 ptrtoint (ptr @g2 to i64), 255
  %b = or i64 ptrtoint (ptr @g3 to i64), 1
  %c = add i64 ptrtoint (ptr getelementptr (i8, ptr @g3, i64 3) to i64), 4
  ret i64 %a
}

define ptr @const_ptr() {
  ret ptr getelementptr (i32, ptr @g3, i64 5)
}
