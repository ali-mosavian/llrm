target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$str1 = internal constant [17 x i8] c"\08\00\0A\00\0A\00 .:-=+*#%@\00"
@$str2 = internal constant [7 x i8] c"\08\00\00\00\00\00\00"
@$str3 = internal constant [12 x i8] c"\08\00\05\00\05\00step \00"
@$str4 = internal constant [15 x i8] c"\08\00\08\00\08\00, limit \00"

define cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = getelementptr i8, ptr @$str1, i16 6
  br label %b3

b3:
  %1 = phi i16 [ 0, %b1 ], [ %46, %b9 ]
  %2 = zext nneg i16 %1 to i32
  %3 = mul i32 %2, 446
  br label %4

b5:
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str3, i16 6))
  tail call cc 1005 addrspace(1) void @N$PQ4(i32 204, i8 zeroext 12)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str4, i16 6))
  tail call cc 1005 addrspace(1) void @N$PU2(i16 40)
  tail call cc 1005 addrspace(1) void @N$PN()
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr getelementptr inbounds nuw (i8, ptr @$str1, i16 6))
  ret i16 0

4:
  %5 = phi i16 [ 0, %b3 ], [ %53, %b12 ]
  %6 = phi ptr [ getelementptr inbounds nuw (i8, ptr @$str2, i16 6), %b3 ], [ %48, %b12 ]
  %7 = zext nneg i16 %5 to i32
  %8 = mul i32 %7, 204
  %9 = add i32 %8, -9011
  br label %10

10:
  %11 = phi i16 [ %31, %20 ], [ 40, %4 ]
  %12 = phi ptr [ %30, %20 ], [ %0, %4 ]
  %13 = phi i32 [ 0, %4 ], [ %28, %20 ]
  %14 = phi i32 [ 0, %4 ], [ %26, %20 ]
  %15 = phi i16 [ 0, %4 ], [ %29, %20 ]
  %16 = tail call i32 @llvm.smul.fix.i32(i32 %13, i32 %13, i32 12)
  %17 = tail call i32 @llvm.smul.fix.i32(i32 %14, i32 %14, i32 12)
  %18 = add i32 %17, %16
  %19 = icmp sgt i32 %18, 16384
  br i1 %19, label %37, label %20

20:
  %21 = mul i32 8192, %13
  %22 = ashr i32 %21, 12
  %23 = mul i32 %22, %14
  %24 = ashr i32 %23, 12
  %25 = sub i32 %24, %3
  %26 = add i32 %25, 4915
  %27 = add i32 %9, %16
  %28 = sub i32 %27, %17
  %29 = add nuw nsw i16 %15, 1
  %30 = getelementptr i8, ptr %12, i16 1
  %31 = add nsw i16 %11, -1
  %exitcond = icmp eq i16 %31, 0
  br i1 %exitcond, label %32, label %10

32:
  %33 = getelementptr i8, ptr %6, i16 -4
  %34 = load i16, ptr %33
  %35 = tail call cc 1005 addrspace(1) ptr @N$BGRW(ptr %6, i16 1, i16 1)
  %36 = getelementptr inbounds i8, ptr %35, i16 %34
  br label %b12

37:
  %38 = getelementptr i8, ptr %6, i16 -4
  %39 = load i16, ptr %38
  %40 = tail call cc 1005 addrspace(1) ptr @N$BGRW(ptr %6, i16 1, i16 1)
  %41 = getelementptr inbounds i8, ptr %40, i16 %39
  %42 = udiv i16 %15, 9
  %43 = mul nsw i16 %42, -9
  %44 = getelementptr i8, ptr %12, i16 %43
  %45 = load i8, ptr %44
  br label %b12

b9:
  tail call cc 1005 addrspace(1) void @N$PS(ptr nonnull %48)
  tail call cc 1005 addrspace(1) void @N$PN()
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr nonnull %48)
  %46 = add nuw nsw i16 %1, 1
  %exitcond2 = icmp eq i16 %46, 22
  br i1 %exitcond2, label %b5, label %b3

b12:
  %47 = phi ptr [ %41, %37 ], [ %36, %32 ]
  %48 = phi ptr [ %40, %37 ], [ %35, %32 ]
  %49 = phi i8 [ %45, %37 ], [ 64, %32 ]
  store i8 %49, ptr %47
  %50 = getelementptr i8, ptr %48, i16 -4
  %51 = load i16, ptr %50
  %52 = getelementptr inbounds i8, ptr %48, i16 %51
  store i8 0, ptr %52
  %53 = add nuw nsw i16 %5, 1
  %exitcond1 = icmp eq i16 %53, 64
  br i1 %exitcond1, label %b9, label %4
}

declare i32 @llvm.smul.fix.i32(i32, i32, i32 immarg) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare i32 @llvm.sdiv.fix.i32(i32, i32, i32 immarg) nocallback nofree nosync nounwind willreturn memory(none)

declare cc 1005 void @N$PS(ptr) addrspace(1)

declare cc 1005 void @N$PQ4(i32, i8) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PU2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$BDRP(ptr) addrspace(1)

declare cc 1005 ptr @N$BGRW(ptr, i16, i16) addrspace(1)

declare cc 1005 void @N$EBND() addrspace(1) noreturn memory(inaccessiblemem: readwrite)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
