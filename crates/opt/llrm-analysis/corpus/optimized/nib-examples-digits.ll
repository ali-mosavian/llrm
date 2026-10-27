target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$str1 = internal constant [12 x i8] c"\08\00\05\00\05\00empty\00"
@$str2 = internal constant [18 x i8] c"\08\00\0B\00\0B\00not a digit\00"
@$str3 = internal constant [14 x i8] c"\08\00\07\00\07\00too big\00"
@$str4 = internal constant [11 x i8] c"\08\00\04\00\04\001234\00"
@$str5 = internal constant [7 x i8] c"\08\00\00\00\00\00\00"
@$str6 = internal constant [11 x i8] c"\08\00\04\00\04\0012x4\00"
@$str7 = internal constant [12 x i8] c"\08\00\05\00\05\0099999\00"
@$str8 = internal constant [18 x i8] c"\08\00\0B\00\0B\00first even \00"

define internal cc 1005 void @report(ptr %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
  %2 = getelementptr i8, ptr %0, i16 -4
  %3 = load i16, ptr %2
  %4 = icmp eq i16 %3, 0
  br i1 %4, label %b7, label %5

5:
  br label %8

6:
  %7 = add nuw i16 %9, 1
  %exitcond = icmp eq i16 %3, %7
  br i1 %exitcond, label %b4, label %8

8:
  %9 = phi i16 [ %7, %6 ], [ 0, %5 ]
  %10 = phi i32 [ %19, %6 ], [ 0, %5 ]
  %11 = getelementptr i8, ptr %0, i16 %9
  %12 = load i8, ptr %11
  %13 = icmp ult i8 %12, 48
  %14 = icmp ugt i8 %12, 57
  %or.cond2 = or i1 %13, %14
  br i1 %or.cond2, label %b10, label %15

15:
  %16 = add nsw i8 %12, -48
  %17 = mul i32 %10, 10
  %18 = zext nneg i8 %16 to i32
  %19 = add i32 %17, %18
  %20 = icmp ugt i32 %19, 65535
  br i1 %20, label %b8, label %6

b2:
  tail call cc 1005 addrspace(1) void @N$PN()
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr null)
  ret void

b4:
  %21 = trunc nuw i32 %19 to i16
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr %0)
  tail call cc 1005 addrspace(1) void @N$PU2(i16 %21)
  br label %b2

b7:
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr nonnull %0)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str1, i16 6))
  br label %b2

b8:
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr nonnull %0)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str3, i16 6))
  br label %b2

b10:
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr nonnull %0)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str2, i16 6))
  br label %b2
}

define cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b2:
  tail call cc 1005 addrspace(1) void @report(ptr getelementptr inbounds nuw (i8, ptr @$str4, i16 6))
  tail call cc 1005 addrspace(1) void @report(ptr getelementptr inbounds nuw (i8, ptr @$str5, i16 6))
  tail call cc 1005 addrspace(1) void @report(ptr getelementptr inbounds nuw (i8, ptr @$str6, i16 6))
  tail call cc 1005 addrspace(1) void @report(ptr getelementptr inbounds nuw (i8, ptr @$str7, i16 6))
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str8, i16 6))
  tail call cc 1005 addrspace(1) void @N$PI2(i16 8)
  tail call cc 1005 addrspace(1) void @N$PN()
  ret i16 0
}

declare void @llvm.memcpy.p1.p0.i16(ptr addrspace(1) noalias nocapture writeonly, ptr noalias nocapture readonly, i16, i1 immarg) nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)

declare cc 1005 void @N$BDRP(ptr) addrspace(1)

declare cc 1005 void @N$PU2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PS(ptr) addrspace(1)

declare cc 1005 void @N$PI2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{i8 0, i8 2}
!6 = !{i8 0, i8 2}
!7 = !{i8 0, i8 3}
!8 = !{i8 0, i8 2}
