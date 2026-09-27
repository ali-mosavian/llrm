target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$f32_3fc00000 = internal constant [4 x i8] c"\00\00\C0?"
@$f64_4002000000000000 = internal constant [8 x i8] c"\00\00\00\00\00\00\02@"

define internal i8 @bool_value() addrspace(1) {
b1:
  ret i8 -1
}

define internal i8 @char_value() addrspace(1) {
b1:
  ret i8 65
}

define internal i8 @i8_value() addrspace(1) {
b1:
  ret i8 -128
}

define internal i8 @u8_value() addrspace(1) {
b1:
  ret i8 -1
}

define internal i16 @i16_value() addrspace(1) {
b1:
  ret i16 -32768
}

define internal i16 @u16_value() addrspace(1) {
b1:
  ret i16 -1
}

define internal i32 @i32_value() addrspace(1) {
b1:
  ret i32 -2147483648
}

define internal i32 @u32_value() addrspace(1) {
b1:
  ret i32 -1
}

define internal float @f32_value() addrspace(1) {
b1:
  %0 = load float, ptr @$f32_3fc00000, !tbaa !2
  ret float %0
}

define internal double @f64_value() addrspace(1) {
b1:
  %0 = load double, ptr @$f64_4002000000000000, !tbaa !2
  %1 = fneg double %0
  ret double %1
}

define internal i32 @unsigned_divide(i32 %0, i32 %1) addrspace(1) {
b1:
  %2 = udiv i32 %0, %1
  ret i32 %2
}

define internal i16 @unsigned_remainder(i16 %0, i16 %1) addrspace(1) {
b1:
  %2 = urem i16 %0, %1
  ret i16 %2
}

define internal i8 @unsigned_less(i16 %0, i16 %1) addrspace(1) {
b1:
  %2 = icmp ult i16 %0, %1
  %3 = sext i1 %2 to i8
  ret i8 %3
}

define internal float @float_product(float %0, float %1) addrspace(1) {
b1:
  %2 = fmul float %0, %1
  ret float %2
}

define internal void @nothing() addrspace(1) {
b1:
  ret void
}

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
