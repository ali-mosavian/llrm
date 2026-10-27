target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$var_bytes = internal global [4 x i8] c"QB*\07"
@$var_samples = internal global [10 x i8] c"\03\00\FF\FF\04\00\FF\FF\05\00", align 2
@$var_corners = internal global [12 x i8] c"\00\00\00\00\1E\00\06\00\06\00\1E\00"
@$var_middle = internal global [4 x i8] zeroinitializer
@$str5 = internal constant [16 x i8] c"\08\00\09\00\09\00checksum \00"
@$str6 = internal constant [16 x i8] c"\08\00\09\00\09\00weighted \00"
@$str7 = internal constant [16 x i8] c"\08\00\09\00\09\00centroid \00"
@$str8 = internal constant [8 x i8] c"\08\00\01\00\01\00,\00"

define cc 1005 i16 @"_weight@3"(i16 %0) addrspace(1) "llrm-fp-eval"="x87-extended" memory(none) willreturn nounwind nofree nosync norecurse {
b1:
  %1 = icmp slt i16 %0, 0
  %2 = mul i16 %0, -3
  %3 = select i1 %1, i16 %2, i16 %0
  ret i16 %3
}

define cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = tail call cc 1005 addrspace(1) i16 @"_checksum@3"(ptr @$var_bytes, i16 4)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str5, i16 6))
  tail call cc 1005 addrspace(1) void @N$PU2(i16 %0)
  tail call cc 1005 addrspace(1) void @N$PN()
  %1 = tail call cc 1005 addrspace(1) i32 @"_weighted_sum@3"(ptr @$var_samples, i16 5)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str6, i16 6))
  tail call cc 1005 addrspace(1) void @N$PI4(i32 %1)
  tail call cc 1005 addrspace(1) void @N$PN()
  tail call cc 1005 addrspace(1) void @"_centroid@3"(ptr @$var_corners, i16 3, ptr @$var_middle)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str7, i16 6))
  %2 = load i16, ptr @$var_middle, !tbaa !2
  tail call cc 1005 addrspace(1) void @N$PI2(i16 %2)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str8, i16 6))
  %3 = load i16, ptr getelementptr inbounds nuw (i8, ptr @$var_middle, i16 2), !tbaa !2
  tail call cc 1005 addrspace(1) void @N$PI2(i16 %3)
  tail call cc 1005 addrspace(1) void @N$PN()
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
