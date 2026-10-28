target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$str1 = internal constant [7 x i8] c"\08\00\00\00\00\00\00"
@$str2 = internal constant [13 x i8] c"\08\00\06\00\06\00Rovers\00"
@$str3 = internal constant [13 x i8] c"\08\00\06\00\06\00United\00"
@$str4 = internal constant [15 x i8] c"\08\00\08\00\08\00Athletic\00"
@$str5 = internal constant [16 x i8] c"\08\00\09\00\09\00 lead on \00"
@$str6 = internal constant [9 x i8] c"\08\00\02\00\02\00: \00"
@$str7 = internal constant [13 x i8] c"\08\00\06\00\06\00 from \00"

define cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca [8 x i8]
  %1 = tail call cc 1005 addrspace(1) ptr @N$BGRW(ptr getelementptr inbounds nuw (i8, ptr @$str1, i16 6), i16 3, i16 2)
  store ptr getelementptr inbounds nuw (i8, ptr @$str2, i16 6), ptr %1
  %2 = getelementptr inbounds nuw i8, ptr %1, i16 2
  store ptr getelementptr inbounds nuw (i8, ptr @$str3, i16 6), ptr %2
  %3 = getelementptr inbounds nuw i8, ptr %1, i16 4
  store ptr getelementptr inbounds nuw (i8, ptr @$str4, i16 6), ptr %3
  %4 = getelementptr i8, ptr %1, i16 -4
  %5 = load i16, ptr %4
  %6 = icmp eq i16 %5, 0
  br i1 %6, label %b6, label %7

7:
  br label %b3

b3:
  %8 = phi ptr [ %1, %7 ], [ %22, %b3 ]
  %9 = phi i16 [ %5, %7 ], [ %21, %b3 ]
  %10 = phi ptr [ %13, %b3 ], [ getelementptr inbounds nuw (i8, ptr @$str1, i16 6), %7 ]
  %11 = getelementptr i8, ptr %10, i16 -4
  %12 = load i16, ptr %11
  %13 = tail call cc 1005 addrspace(1) ptr @N$BGRW(ptr nonnull %10, i16 1, i16 8)
  %14 = shl i16 %12, 3
  %15 = getelementptr inbounds i8, ptr %13, i16 %14
  %16 = load ptr, ptr %8
  %17 = tail call cc 1005 addrspace(1) ptr @N$BCLN(ptr %16, i16 1)
  store ptr %17, ptr %15
  %18 = getelementptr i8, ptr %15, i16 2
  store i16 0, ptr %18
  %19 = getelementptr i8, ptr %15, i16 4
  store i16 0, ptr %19
  %20 = getelementptr i8, ptr %15, i16 6
  store i16 0, ptr %20
  %21 = add i16 %9, -1
  %22 = getelementptr i8, ptr %8, i16 2
  %exitcond5 = icmp eq i16 %21, 0
  br i1 %exitcond5, label %b7, label %b3

b6:
  %23 = phi ptr [ %13, %b7 ], [ getelementptr inbounds nuw (i8, ptr @$str1, i16 6), %b1 ], [ %13, %b10 ]
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr nonnull %1)
  %24 = getelementptr i8, ptr %23, i16 -4
  %25 = load i16, ptr %24
  %26 = icmp eq i16 %25, 0
  br i1 %26, label %b12, label %36

b7:
  %27 = getelementptr i8, ptr %1, i16 -4
  %28 = load i16, ptr %27
  %29 = icmp eq i16 %28, 0
  br i1 %29, label %b6, label %30

30:
  br label %b10

b10:
  %31 = phi ptr [ %1, %30 ], [ %35, %b10 ]
  %32 = phi i16 [ %28, %30 ], [ %34, %b10 ]
  %33 = load ptr, ptr %31
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr %33)
  %34 = add i16 %32, -1
  %35 = getelementptr i8, ptr %31, i16 2
  %exitcond4 = icmp eq i16 %34, 0
  br i1 %exitcond4, label %b6, label %b10

b12:
  tail call cc 1005 addrspace(1) void @N$EBND()
  unreachable

36:
  %37 = getelementptr inbounds nuw i8, ptr %23, i16 2
  %38 = load i16, ptr %37
  %39 = add i16 %38, 1
  store i16 %39, ptr %37
  %40 = getelementptr inbounds nuw i8, ptr %23, i16 4
  %41 = load i16, ptr %40
  %42 = add i16 %41, 1
  store i16 %42, ptr %40
  %43 = icmp eq i16 %25, 1
  br i1 %43, label %b16, label %44

b16:
  tail call cc 1005 addrspace(1) void @N$EBND()
  unreachable

44:
  %45 = getelementptr inbounds nuw i8, ptr %23, i16 10
  %46 = load i16, ptr %45
  %47 = add i16 %46, 1
  store i16 %47, ptr %45
  %48 = getelementptr inbounds nuw i8, ptr %23, i16 14
  %49 = load i16, ptr %48
  %50 = add i16 %49, 1
  store i16 %50, ptr %48
  %51 = icmp ugt i16 %25, 2
  br i1 %51, label %52, label %b20

b20:
  tail call cc 1005 addrspace(1) void @N$EBND()
  unreachable

52:
  %53 = getelementptr inbounds nuw i8, ptr %23, i16 20
  %54 = load i16, ptr %53
  %55 = add i16 %54, 1
  store i16 %55, ptr %53
  %56 = getelementptr inbounds nuw i8, ptr %23, i16 22
  %57 = load i16, ptr %56
  %58 = add i16 %57, 1
  store i16 %58, ptr %56
  br label %59

59:
  %60 = phi i16 [ %76, %59 ], [ 4, %52 ]
  %61 = phi i16 [ %75, %59 ], [ %25, %52 ]
  %62 = phi ptr [ getelementptr inbounds nuw (i8, ptr @$str1, i16 6), %52 ], [ %65, %59 ]
  %63 = getelementptr i8, ptr %62, i16 -4
  %64 = load i16, ptr %63
  %65 = tail call cc 1005 addrspace(1) ptr @N$BGRW(ptr nonnull %62, i16 1, i16 2)
  %66 = shl i16 %64, 1
  %67 = getelementptr inbounds i8, ptr %65, i16 %66
  %68 = getelementptr i8, ptr %23, i16 %60
  %69 = getelementptr i8, ptr %68, i16 -2
  %70 = load i16, ptr %69
  %71 = mul i16 %70, 3
  %72 = getelementptr i8, ptr %23, i16 %60
  %73 = load i16, ptr %72
  %74 = add i16 %71, %73
  store i16 %74, ptr %67
  %75 = add i16 %61, -1
  %76 = add i16 %60, 8
  %exitcond3 = icmp eq i16 %75, 0
  br i1 %exitcond3, label %77, label %59

77:
  %78 = getelementptr i8, ptr %65, i16 -4
  %79 = load i16, ptr %78
  %80 = icmp eq i16 %79, 0
  br i1 %80, label %83, label %81

81:
  %82 = load i16, ptr %65
  br label %84

83:
  tail call cc 1005 addrspace(1) void @N$EBND()
  unreachable

84:
  %85 = phi i16 [ 0, %81 ], [ %94, %84 ]
  %86 = phi i16 [ 0, %81 ], [ %93, %84 ]
  %87 = phi i16 [ %82, %81 ], [ %92, %84 ]
  %88 = shl i16 %85, 1
  %89 = getelementptr i8, ptr %65, i16 %88
  %90 = load i16, ptr %89
  %91 = icmp sgt i16 %90, %87
  %92 = tail call i16 @llvm.smax.i16(i16 %90, i16 %87)
  %93 = select i1 %91, i16 %85, i16 %86
  %94 = add nuw i16 %85, 1
  %exitcond = icmp eq i16 %79, %94
  br i1 %exitcond, label %95, label %84

95:
  %96 = getelementptr i8, ptr %23, i16 -4
  %97 = load i16, ptr %96
  %98 = icmp ugt i16 %97, %93
  br i1 %98, label %b27, label %b28

b27:
  %99 = shl i16 %93, 3
  %100 = getelementptr inbounds i8, ptr %23, i16 %99
  %101 = load ptr, ptr %100
  tail call cc 1005 addrspace(1) void @N$PS(ptr %101)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str5, i16 6))
  tail call cc 1005 addrspace(1) void @N$PI2(i16 %92)
  tail call cc 1005 addrspace(1) void @N$PN()
  %102 = getelementptr i8, ptr %65, i16 -4
  %103 = load i16, ptr %102
  %104 = icmp eq i16 %103, 0
  br i1 %104, label %b33, label %105

105:
  %106 = addrspacecast ptr %0 to ptr addrspace(1)
  %107 = getelementptr i8, ptr %23, i16 6
  br label %b31

b28:
  tail call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b31:
  %108 = phi ptr [ %119, %b36 ], [ %107, %105 ]
  %109 = phi i16 [ 0, %105 ], [ %118, %b36 ]
  %110 = shl i16 %109, 1
  %111 = getelementptr i8, ptr %65, i16 %110
  %112 = load i16, ptr %111
  %113 = icmp sgt i16 %112, 1
  br i1 %113, label %b34, label %b36

b33:
  call cc 1005 addrspace(1) void @N$BDRP(ptr nonnull %65)
  call cc 1005 addrspace(1) void @N$BDRP(ptr null)
  %114 = icmp eq ptr %23, null
  br i1 %114, label %b40, label %b41

b34:
  %115 = getelementptr i8, ptr %23, i16 -4
  %116 = load i16, ptr %115
  %117 = icmp ult i16 %109, %116
  br i1 %117, label %120, label %b39

b36:
  %118 = add nuw i16 %109, 1
  %119 = getelementptr i8, ptr %108, i16 8
  %exitcond2 = icmp eq i16 %103, %118
  br i1 %exitcond2, label %b33, label %b31

120:
  %121 = getelementptr i8, ptr %108, i16 -6
  %122 = load ptr, ptr %121
  %123 = getelementptr i8, ptr %108, i16 -4
  %124 = load i16, ptr %123
  %125 = getelementptr i8, ptr %108, i16 -2
  %126 = load i16, ptr %125
  %127 = load i16, ptr %108
  %128 = call cc 1005 addrspace(1) ptr @N$BCLN(ptr %122, i16 1)
  %129 = getelementptr i8, ptr %128, i16 -4
  %130 = load i16, ptr %129
  %131 = addrspacecast ptr %128 to ptr addrspace(1)
  store i16 %130, ptr %0, !tbaa !2
  %132 = getelementptr i8, ptr %0, i16 2
  store i16 %130, ptr %132, !tbaa !2
  %133 = getelementptr i8, ptr %0, i16 4
  store ptr addrspace(1) %131, ptr %133, !tbaa !2
  %134 = mul i16 %124, 3
  %135 = add i16 %134, %126
  %136 = add i16 %126, %124
  %137 = add i16 %136, %127
  call cc 1005 addrspace(1) void @N$PV(ptr addrspace(1) %106)
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str6, i16 6))
  call cc 1005 addrspace(1) void @N$PI2(i16 %135)
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str7, i16 6))
  call cc 1005 addrspace(1) void @N$PI2(i16 %137)
  call cc 1005 addrspace(1) void @N$PN()
  call cc 1005 addrspace(1) void @N$BDRP(ptr %128)
  br label %b36

b39:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b40:
  call cc 1005 addrspace(1) void @N$BDRP(ptr %23)
  ret i16 0

b41:
  %138 = getelementptr i8, ptr %23, i16 -4
  %139 = load i16, ptr %138
  %140 = icmp eq i16 %139, 0
  br i1 %140, label %b40, label %141

141:
  br label %b44

b44:
  %142 = phi ptr [ %23, %141 ], [ %146, %b44 ]
  %143 = phi i16 [ %139, %141 ], [ %145, %b44 ]
  %144 = load ptr, ptr %142
  call cc 1005 addrspace(1) void @N$BDRP(ptr %144)
  %145 = add i16 %143, -1
  %146 = getelementptr i8, ptr %142, i16 8
  %exitcond1 = icmp eq i16 %145, 0
  br i1 %exitcond1, label %b40, label %b44
}

declare cc 1005 ptr @N$BGRW(ptr, i16, i16) addrspace(1)

declare cc 1005 ptr @N$BCLN(ptr, i16) addrspace(1)

declare cc 1005 void @N$BDRP(ptr) addrspace(1)

declare cc 1005 void @N$EBND() addrspace(1) noreturn memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PS(ptr) addrspace(1)

declare cc 1005 void @N$PI2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PV(ptr addrspace(1)) addrspace(1)

declare i16 @llvm.smax.i16(i16, i16) nocallback nofree nosync nounwind speculatable willreturn memory(none)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{i16 0, i16 -32768}
