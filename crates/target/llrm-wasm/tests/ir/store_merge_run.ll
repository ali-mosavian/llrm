target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32-unknown-unknown"
@s = global [48 x i16] zeroinitializer, align 16
@d = global [48 x i16] zeroinitializer, align 16
define void @copy() {
  %l0 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i32 0), align 2
  store i16 %l0, ptr getelementptr inbounds nuw (i8, ptr @d, i32 0), align 2
  %l1 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i32 2), align 2
  store i16 %l1, ptr getelementptr inbounds nuw (i8, ptr @d, i32 2), align 2
  %l2 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i32 4), align 2
  store i16 %l2, ptr getelementptr inbounds nuw (i8, ptr @d, i32 4), align 2
  %l3 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i32 6), align 2
  store i16 %l3, ptr getelementptr inbounds nuw (i8, ptr @d, i32 6), align 2
  %l4 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i32 8), align 2
  store i16 %l4, ptr getelementptr inbounds nuw (i8, ptr @d, i32 8), align 2
  %l5 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i32 10), align 2
  store i16 %l5, ptr getelementptr inbounds nuw (i8, ptr @d, i32 10), align 2
  %l6 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i32 12), align 2
  store i16 %l6, ptr getelementptr inbounds nuw (i8, ptr @d, i32 12), align 2
  %l7 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i32 14), align 2
  store i16 %l7, ptr getelementptr inbounds nuw (i8, ptr @d, i32 14), align 2
  %l8 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i32 16), align 2
  store i16 %l8, ptr getelementptr inbounds nuw (i8, ptr @d, i32 16), align 2
  %l9 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i32 18), align 2
  store i16 %l9, ptr getelementptr inbounds nuw (i8, ptr @d, i32 18), align 2
  %l10 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i32 20), align 2
  store i16 %l10, ptr getelementptr inbounds nuw (i8, ptr @d, i32 20), align 2
  %l11 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i32 22), align 2
  store i16 %l11, ptr getelementptr inbounds nuw (i8, ptr @d, i32 22), align 2
  %l12 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i32 24), align 2
  store i16 %l12, ptr getelementptr inbounds nuw (i8, ptr @d, i32 24), align 2
  %l13 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i32 26), align 2
  store i16 %l13, ptr getelementptr inbounds nuw (i8, ptr @d, i32 26), align 2
  %l14 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i32 28), align 2
  store i16 %l14, ptr getelementptr inbounds nuw (i8, ptr @d, i32 28), align 2
  %l15 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i32 30), align 2
  store i16 %l15, ptr getelementptr inbounds nuw (i8, ptr @d, i32 30), align 2
  %l16 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i32 32), align 2
  store i16 %l16, ptr getelementptr inbounds nuw (i8, ptr @d, i32 32), align 2
  %l17 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i32 34), align 2
  store i16 %l17, ptr getelementptr inbounds nuw (i8, ptr @d, i32 34), align 2
  %l18 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i32 36), align 2
  store i16 %l18, ptr getelementptr inbounds nuw (i8, ptr @d, i32 36), align 2
  %l19 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i32 38), align 2
  store i16 %l19, ptr getelementptr inbounds nuw (i8, ptr @d, i32 38), align 2
  %l20 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i32 40), align 2
  store i16 %l20, ptr getelementptr inbounds nuw (i8, ptr @d, i32 40), align 2
  %l21 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i32 42), align 2
  store i16 %l21, ptr getelementptr inbounds nuw (i8, ptr @d, i32 42), align 2
  %l22 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i32 44), align 2
  store i16 %l22, ptr getelementptr inbounds nuw (i8, ptr @d, i32 44), align 2
  %l23 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i32 46), align 2
  store i16 %l23, ptr getelementptr inbounds nuw (i8, ptr @d, i32 46), align 2
  %l24 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i32 48), align 2
  store i16 %l24, ptr getelementptr inbounds nuw (i8, ptr @d, i32 48), align 2
  %l25 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i32 50), align 2
  store i16 %l25, ptr getelementptr inbounds nuw (i8, ptr @d, i32 50), align 2
  %l26 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i32 52), align 2
  store i16 %l26, ptr getelementptr inbounds nuw (i8, ptr @d, i32 52), align 2
  %l27 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i32 54), align 2
  store i16 %l27, ptr getelementptr inbounds nuw (i8, ptr @d, i32 54), align 2
  %l28 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i32 56), align 2
  store i16 %l28, ptr getelementptr inbounds nuw (i8, ptr @d, i32 56), align 2
  %l29 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i32 58), align 2
  store i16 %l29, ptr getelementptr inbounds nuw (i8, ptr @d, i32 58), align 2
  ret void
}
