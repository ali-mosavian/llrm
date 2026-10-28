target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$var_bytes = internal global [4 x i8] c"QB*\07"
@$var_samples = internal global [10 x i8] c"\03\00\FF\FF\04\00\FF\FF\05\00", align 2
@$var_corners = internal global [12 x i8] c"\00\00\00\00\1E\00\06\00\06\00\1E\00"
@$var_middle = internal global [4 x i8] zeroinitializer
@$str5 = internal constant [16 x i8] c"\08\00\09\00\09\00checksum \00"
@$str6 = internal constant [16 x i8] c"\08\00\09\00\09\00weighted \00"
@$str7 = internal constant [16 x i8] c"\08\00\09\00\09\00centroid \00"
@$str8 = internal constant [8 x i8] c"\08\00\01\00\01\00,\00"

define cc 1005 i16 @"_weight@3"(i16 %0) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %1 = icmp slt i16 %0, 0
  br i1 %1, label %b2, label %b3

b2:
  %2 = sub i16 0, %0
  %3 = mul i16 %2, 3
  ret i16 %3

b3:
  br label %b4

b4:
  ret i16 %0
}

define internal cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca i32
  %1 = alloca i16
  %2 = call cc 1005 addrspace(1) i16 @"_checksum@3"(ptr @$var_bytes, i16 4)
  store i16 %2, ptr %1, !tbaa !2
  %3 = getelementptr i8, ptr @$str5, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %3)
  %4 = load i16, ptr %1, !tbaa !2
  call cc 1005 addrspace(1) void @N$PU2(i16 %4)
  call cc 1005 addrspace(1) void @N$PN()
  %5 = call cc 1005 addrspace(1) i32 @"_weighted_sum@3"(ptr @$var_samples, i16 5)
  store i32 %5, ptr %0, !tbaa !2
  %6 = getelementptr i8, ptr @$str6, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %6)
  %7 = load i32, ptr %0, !tbaa !2
  call cc 1005 addrspace(1) void @N$PI4(i32 %7)
  call cc 1005 addrspace(1) void @N$PN()
  call cc 1005 addrspace(1) void @"_centroid@3"(ptr @$var_corners, i16 3, ptr @$var_middle)
  %8 = getelementptr i8, ptr @$str7, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %8)
  %9 = load i16, ptr @$var_middle, !tbaa !2
  call cc 1005 addrspace(1) void @N$PI2(i16 %9)
  %10 = getelementptr i8, ptr @$str8, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %10)
  %11 = getelementptr inbounds i8, ptr @$var_middle, i16 2
  %12 = load i16, ptr %11, !tbaa !2
  call cc 1005 addrspace(1) void @N$PI2(i16 %12)
  call cc 1005 addrspace(1) void @N$PN()
  ret i16 0
}

declare cc 1005 i16 @"_checksum@3"(ptr, i16) addrspace(1)

declare cc 1005 void @N$PS(ptr) addrspace(1)

declare cc 1005 void @N$PU2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 i32 @"_weighted_sum@3"(ptr, i16) addrspace(1)

declare cc 1005 void @N$PI4(i32) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @"_centroid@3"(ptr, i16, ptr) addrspace(1)

declare cc 1005 void @N$PI2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
