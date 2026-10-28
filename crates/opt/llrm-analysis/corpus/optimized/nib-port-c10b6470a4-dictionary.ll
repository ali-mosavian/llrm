target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$str1 = internal constant [7 x i8] c"\08\00\00\00\00\00\00"

define cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca [8 x i8]
  store i32 131073, ptr %0
  %1 = getelementptr inbounds nuw i8, ptr %0, i16 4
  store i32 196609, ptr %1
  br label %2

2:
  %3 = phi ptr [ getelementptr inbounds nuw (i8, ptr @$str1, i16 6), %b1 ], [ %5, %b31 ]
  %4 = phi i16 [ 0, %b1 ], [ %52, %b31 ]
  %5 = tail call cc 1005 addrspace(1) ptr @N$DRES(ptr nonnull %3, i16 6)
  %6 = getelementptr inbounds nuw [2 x i8], ptr %0, i16 %4
  %7 = load i16, ptr %6, !tbaa !2
  %8 = or i16 %7, 1
  %9 = getelementptr i8, ptr %5, i16 -4
  %10 = load i16, ptr %9
  %11 = icmp eq i16 %10, 0
  br i1 %11, label %b22, label %16

12:
  %13 = getelementptr i8, ptr %5, i16 -4
  %14 = load i16, ptr %13
  %15 = icmp eq i16 %14, 0
  br i1 %15, label %b93, label %b33

16:
  %17 = add i16 %10, -1
  %18 = and i16 %17, %8
  br label %b12

b12:
  %19 = phi i16 [ %18, %16 ], [ %25, %b16 ]
  %20 = mul i16 %19, 6
  %21 = getelementptr inbounds i8, ptr %5, i16 %20
  %22 = load i16, ptr %21
  %23 = icmp eq i16 %22, 0
  br i1 %23, label %b22, label %b18

b16:
  %24 = add nuw i16 %19, 1
  %25 = and i16 %24, %17
  br label %b12

b18:
  %26 = icmp eq i16 %22, %8
  br i1 %26, label %27, label %b16

27:
  %28 = getelementptr i8, ptr %5, i16 %20
  %29 = getelementptr i8, ptr %28, i16 2
  %30 = load i16, ptr %29
  %31 = icmp eq i16 %30, %7
  br i1 %31, label %b30, label %b16

b22:
  %.ph3 = phi i16 [ 0, %2 ], [ %19, %b12 ]
  %32 = icmp ult i16 %.ph3, %10
  br i1 %32, label %b25, label %b26

b25:
  %33 = mul i16 %.ph3, 6
  %34 = getelementptr inbounds i8, ptr %5, i16 %33
  store i16 %8, ptr %34
  %35 = getelementptr i8, ptr %5, i16 -4
  %36 = load i16, ptr %35
  %37 = icmp ult i16 %.ph3, %36
  br i1 %37, label %b29, label %b28

b26:
  tail call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b28:
  tail call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b29:
  %38 = getelementptr i8, ptr %5, i16 %33
  %39 = getelementptr i8, ptr %38, i16 2
  store i16 %7, ptr %39
  %40 = getelementptr i8, ptr %5, i16 -2
  %41 = load i16, ptr %40
  %42 = add i16 %41, 1
  store i16 %42, ptr %40
  %43 = getelementptr i8, ptr %5, i16 -4
  %44 = load i16, ptr %43
  br label %b30

b30:
  %45 = phi i16 [ %44, %b29 ], [ %10, %27 ]
  %46 = phi i16 [ %.ph3, %b29 ], [ %19, %27 ]
  %47 = icmp ult i16 %46, %45
  br i1 %47, label %b31, label %b32

b31:
  %48 = mul i16 %46, 6
  %49 = getelementptr inbounds i8, ptr %5, i16 %48
  %50 = mul i16 %7, 10
  %51 = getelementptr i8, ptr %49, i16 4
  store i16 %50, ptr %51
  %52 = add nuw nsw i16 %4, 1
  %exitcond = icmp eq i16 %52, 4
  br i1 %exitcond, label %12, label %2

b32:
  tail call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b33:
  %53 = add i16 %14, -1
  %54 = and i16 %53, 1
  br label %b36

b36:
  %55 = phi i16 [ %54, %b33 ], [ %64, %b43 ]
  %56 = icmp ult i16 %55, %14
  br i1 %56, label %b39, label %b40

b39:
  %57 = mul i16 %55, 6
  %58 = getelementptr inbounds i8, ptr %5, i16 %57
  %59 = load i16, ptr %58
  %60 = icmp eq i16 %59, 0
  br i1 %60, label %b54, label %61

61:
  %62 = icmp eq i16 %59, 1
  br i1 %62, label %65, label %b43

b40:
  tail call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b43:
  %63 = add nuw i16 %55, 1
  %64 = and i16 %63, %53
  br label %b36

65:
  %66 = getelementptr i8, ptr %5, i16 %57
  %67 = getelementptr i8, ptr %66, i16 2
  %68 = load i16, ptr %67
  %69 = icmp eq i16 %68, 1
  br i1 %69, label %b52, label %b43

b52:
  %70 = getelementptr i8, ptr %5, i16 %57
  %71 = getelementptr i8, ptr %70, i16 4
  %72 = load i16, ptr %71
  br label %b54

b54:
  %.ph4 = phi i16 [ %72, %b52 ], [ %59, %b39 ]
  %73 = and i16 %53, 3
  br label %b57

b57:
  %74 = phi i16 [ %73, %b54 ], [ %83, %b64 ]
  %75 = icmp ult i16 %74, %14
  br i1 %75, label %b60, label %b61

b60:
  %76 = mul i16 %74, 6
  %77 = getelementptr inbounds i8, ptr %5, i16 %76
  %78 = load i16, ptr %77
  %79 = icmp eq i16 %78, 0
  br i1 %79, label %b75, label %80

80:
  %81 = icmp eq i16 %78, 3
  br i1 %81, label %84, label %b64

b61:
  tail call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b64:
  %82 = add nuw i16 %74, 1
  %83 = and i16 %82, %53
  br label %b57

84:
  %85 = getelementptr i8, ptr %5, i16 %76
  %86 = getelementptr i8, ptr %85, i16 2
  %87 = load i16, ptr %86
  %88 = icmp eq i16 %87, 3
  br i1 %88, label %b73, label %b64

b73:
  %89 = getelementptr i8, ptr %5, i16 %76
  %90 = getelementptr i8, ptr %89, i16 4
  %91 = load i16, ptr %90
  br label %b75

b75:
  %.ph5 = phi i16 [ %91, %b73 ], [ %78, %b60 ]
  %92 = and i16 %53, 9
  br label %b78

b78:
  %93 = phi i16 [ %92, %b75 ], [ %102, %b85 ]
  %94 = icmp ult i16 %93, %14
  br i1 %94, label %b81, label %b82

b81:
  %95 = mul i16 %93, 6
  %96 = getelementptr inbounds i8, ptr %5, i16 %95
  %97 = load i16, ptr %96
  %98 = icmp eq i16 %97, 0
  br i1 %98, label %b93, label %99

99:
  %100 = icmp eq i16 %97, 9
  br i1 %100, label %103, label %b85

b82:
  tail call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b85:
  %101 = add nuw i16 %93, 1
  %102 = and i16 %101, %53
  br label %b78

103:
  %104 = getelementptr i8, ptr %5, i16 %95
  %105 = getelementptr i8, ptr %104, i16 2
  %106 = load i16, ptr %105
  %107 = icmp eq i16 %106, 9
  br i1 %107, label %b94, label %b85

b93:
  %108 = phi i16 [ %.ph5, %b94 ], [ %.ph5, %b81 ], [ 0, %12 ]
  %109 = phi i16 [ %.ph4, %b94 ], [ %.ph4, %b81 ], [ 0, %12 ]
  %110 = phi i16 [ %115, %b94 ], [ 5, %b81 ], [ 5, %12 ]
  %111 = add i16 %109, %108
  %112 = add i16 %111, %110
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr nonnull %5)
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr null)
  ret i16 %112

b94:
  %113 = getelementptr i8, ptr %5, i16 %95
  %114 = getelementptr i8, ptr %113, i16 4
  %115 = load i16, ptr %114
  br label %b93
}

declare cc 1005 ptr @N$DRES(ptr, i16) addrspace(1)

declare cc 1005 void @N$EBND() addrspace(1) noreturn memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$BDRP(ptr) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{i8 0, i8 2}
!6 = !{i8 0, i8 2}
!7 = !{i8 0, i8 2}
!8 = !{i8 0, i8 2}
!9 = !{i8 0, i8 2}
!10 = !{i8 0, i8 2}
!11 = !{i8 0, i8 2}
