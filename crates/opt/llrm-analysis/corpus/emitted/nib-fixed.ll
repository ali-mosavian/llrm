target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$str1 = internal constant [13 x i8] c"\08\00\06\00\06\00fixed=\00"

define internal cc 1005 i16 @product(i16 %0, i16 %1) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %2 = sext i16 %0 to i32
  %3 = sext i16 %1 to i32
  %4 = mul i32 %2, %3
  %5 = ashr i32 %4, 8
  %6 = trunc i32 %5 to i16
  ret i16 %6
}

define internal cc 1005 i32 @quotient(i32 %0, i32 %1) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %2 = sext i32 %0 to i64
  %3 = sext i32 %1 to i64
  %4 = shl i64 %2, 16
  %5 = sdiv i64 %4, %3
  %6 = trunc i64 %5 to i32
  ret i32 %6
}

define internal cc 1005 i32 @fixed_literals() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca i32
  store i32 98304, ptr %0, !tbaa !2
  store i32 147456, ptr %0, !tbaa !2
  %1 = load i32, ptr %0, !tbaa !2
  call cc 1005 addrspace(1) void @N$PQ4(i32 %1, i8 zeroext 16)
  call cc 1005 addrspace(1) void @N$PN()
  %2 = getelementptr i8, ptr @$str1, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %2)
  %3 = load i32, ptr %0, !tbaa !2
  call cc 1005 addrspace(1) void @N$PQ4(i32 %3, i8 zeroext 16)
  call cc 1005 addrspace(1) void @N$PN()
  %4 = load i32, ptr %0, !tbaa !2
  ret i32 %4
}

define internal cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = call cc 1005 addrspace(1) i32 @fixed_literals()
  ret i16 0
}

declare i32 @llvm.sdiv.fix.i32(i32, i32, i32 immarg) nocallback nofree nosync nounwind willreturn memory(none)

declare cc 1005 void @N$PQ4(i32, i8) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PS(ptr) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
