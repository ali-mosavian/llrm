target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$str1 = internal constant [9 x i8] c"\08\00\02\00\02\00 (\00"
@$str2 = internal constant [8 x i8] c"\08\00\01\00\01\00)\00"
@$str3 = internal constant [13 x i8] c"\08\00\06\00\06\00master\00"
@$str4 = internal constant [13 x i8] c"\08\00\06\00\06\00expert\00"
@$str5 = internal constant [11 x i8] c"\08\00\04\00\04\00club\00"
@$str6 = internal constant [7 x i8] c"\08\00\00\00\00\00\00"
@$str7 = internal constant [10 x i8] c"\08\00\03\00\03\00ada\00"
@$str8 = internal constant [10 x i8] c"\08\00\03\00\03\00bob\00"
@$str9 = internal constant [9 x i8] c"\08\00\02\00\02\00cy\00"
@$str12 = internal constant [22 x i8] c"\08\00\0F\00\0F\00refused rating \00"
@$str13 = internal constant [9 x i8] c"\08\00\02\00\02\00 *\00"
@$str14 = internal constant [8 x i8] c"\08\00\01\00\01\00 \00"
@$str15 = internal constant [26 x i8] c"\08\00\13\00\13\00 above 1500, first \00"
@$str16 = internal constant [11 x i8] c"\08\00\04\00\04\00 at \00"

define cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca [8 x i8]
  %1 = tail call cc 1005 addrspace(1) ptr @N$BGRW(ptr getelementptr inbounds nuw (i8, ptr @$str6, i16 6), i16 3, i16 4)
  store ptr getelementptr inbounds nuw (i8, ptr @$str7, i16 6), ptr %1
  %2 = getelementptr i8, ptr %1, i16 2
  store i16 2150, ptr %2
  %3 = getelementptr inbounds nuw i8, ptr %1, i16 4
  store ptr getelementptr inbounds nuw (i8, ptr @$str8, i16 6), ptr %3
  %4 = getelementptr i8, ptr %1, i16 6
  store i16 1480, ptr %4
  %5 = getelementptr inbounds nuw i8, ptr %1, i16 8
  store ptr getelementptr inbounds nuw (i8, ptr @$str9, i16 6), ptr %5
  %6 = getelementptr i8, ptr %1, i16 10
  store i16 1620, ptr %6
  %7 = getelementptr i8, ptr %1, i16 -4
  %8 = load i16, ptr %7
  %9 = icmp eq i16 %8, 0
  br i1 %9, label %b11, label %10

10:
  %11 = addrspacecast ptr %0 to ptr addrspace(1)
  %12 = getelementptr i8, ptr %1, i16 2
  br label %13

13:
  %14 = phi ptr [ %39, %36 ], [ %12, %10 ]
  %15 = phi i16 [ %38, %36 ], [ %8, %10 ]
  %16 = phi ptr [ getelementptr inbounds nuw (i8, ptr @$str6, i16 6), %10 ], [ %37, %36 ]
  %17 = getelementptr i8, ptr %14, i16 -2
  %18 = load ptr, ptr %17
  %19 = getelementptr i8, ptr %18, i16 -4
  %20 = load i16, ptr %19
  %21 = addrspacecast ptr %18 to ptr addrspace(1)
  store i16 %20, ptr %0, !tbaa !2
  %22 = getelementptr i8, ptr %0, i16 2
  store i16 %20, ptr %22, !tbaa !2
  %23 = getelementptr i8, ptr %0, i16 4
  store ptr addrspace(1) %21, ptr %23, !tbaa !2
  %24 = load i16, ptr %14
  %25 = icmp slt i16 %24, 0
  br i1 %25, label %36, label %26

26:
  %27 = call cc 1005 addrspace(1) ptr @N$VCPY(ptr addrspace(1) %11)
  %28 = ptrtoint ptr %27 to i16
  %29 = inttoptr i16 %28 to ptr
  %30 = getelementptr i8, ptr %16, i16 -4
  %31 = load i16, ptr %30
  %32 = call cc 1005 addrspace(1) ptr @N$BGRW(ptr %16, i16 1, i16 4)
  %33 = shl i16 %31, 2
  %34 = getelementptr inbounds i8, ptr %32, i16 %33
  store ptr %29, ptr %34
  %35 = getelementptr i8, ptr %34, i16 2
  store i16 %24, ptr %35
  call cc 1005 addrspace(1) void @N$BDRP(ptr null)
  br label %36

36:
  %37 = phi ptr [ %32, %26 ], [ %16, %13 ]
  %38 = add i16 %15, -1
  %39 = getelementptr i8, ptr %14, i16 4
  %exitcond5 = icmp eq i16 %38, 0
  br i1 %exitcond5, label %b7, label %13

b7:
  %40 = getelementptr i8, ptr %1, i16 -4
  %41 = load i16, ptr %40
  %42 = icmp eq i16 %41, 0
  br i1 %42, label %b11, label %43

43:
  br label %b10

b10:
  %44 = phi ptr [ %1, %43 ], [ %48, %b10 ]
  %45 = phi i16 [ %41, %43 ], [ %47, %b10 ]
  %46 = load ptr, ptr %44
  call cc 1005 addrspace(1) void @N$BDRP(ptr %46)
  %47 = add i16 %45, -1
  %48 = getelementptr i8, ptr %44, i16 4
  %exitcond4 = icmp eq i16 %47, 0
  br i1 %exitcond4, label %b11, label %b10

b11:
  %49 = phi ptr [ %37, %b7 ], [ getelementptr inbounds nuw (i8, ptr @$str6, i16 6), %b1 ], [ %37, %b10 ]
  call cc 1005 addrspace(1) void @N$BDRP(ptr nonnull %1)
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str12, i16 6))
  call cc 1005 addrspace(1) void @N$PI2(i16 -3)
  call cc 1005 addrspace(1) void @N$PN()
  %50 = getelementptr i8, ptr %49, i16 -4
  %51 = load i16, ptr %50
  %52 = icmp ugt i16 %51, 1
  br i1 %52, label %53, label %b16

53:
  %54 = getelementptr i8, ptr %49, i16 6
  %55 = load i16, ptr %54
  %56 = add i16 %55, 25
  store i16 %56, ptr %54
  br label %59

57:
  %58 = icmp ult i16 %73, %51
  br i1 %58, label %77, label %76

59:
  %60 = phi i16 [ 0, %53 ], [ %73, %63 ]
  %61 = phi i16 [ 0, %53 ], [ %74, %63 ]
  %62 = icmp ult i16 %60, %51
  br i1 %62, label %63, label %75

63:
  %64 = shl i16 %61, 2
  %65 = getelementptr i8, ptr %49, i16 %64
  %66 = getelementptr i8, ptr %65, i16 2
  %67 = load i16, ptr %66
  %68 = shl i16 %60, 2
  %69 = getelementptr inbounds i8, ptr %49, i16 %68
  %70 = getelementptr i8, ptr %69, i16 2
  %71 = load i16, ptr %70
  %72 = icmp sgt i16 %67, %71
  %73 = select i1 %72, i16 %61, i16 %60
  %74 = add nuw i16 %61, 1
  %exitcond = icmp eq i16 %51, %74
  br i1 %exitcond, label %57, label %59

75:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

76:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

77:
  %78 = shl i16 %73, 2
  %79 = getelementptr inbounds i8, ptr %49, i16 %78
  %80 = addrspacecast ptr %79 to ptr addrspace(1)
  br label %81

b16:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

81:
  %82 = phi ptr [ %97, %81 ], [ %49, %77 ]
  %83 = phi i16 [ %96, %81 ], [ %51, %77 ]
  %84 = addrspacecast ptr %82 to ptr addrspace(1)
  %85 = icmp eq ptr addrspace(1) %80, %84
  %86 = select i1 %85, ptr getelementptr inbounds nuw (i8, ptr @$str13, i16 6), ptr getelementptr inbounds nuw (i8, ptr @$str6, i16 6)
  call cc 1005 addrspace(1) void @N$PBEG()
  %87 = load ptr, ptr %82
  call cc 1005 addrspace(1) void @N$PS(ptr %87)
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str1, i16 6))
  %88 = getelementptr i8, ptr %82, i16 2
  %89 = load i16, ptr %88
  call cc 1005 addrspace(1) void @N$PI2(i16 %89)
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str2, i16 6))
  %90 = call cc 1005 addrspace(1) ptr @N$PEND()
  %91 = load i16, ptr %88
  %92 = icmp sgt i16 %91, 1999
  %93 = icmp sgt i16 %91, 1499
  %94 = select i1 %93, ptr getelementptr inbounds nuw (i8, ptr @$str4, i16 6), ptr getelementptr inbounds nuw (i8, ptr @$str5, i16 6)
  %95 = select i1 %92, ptr getelementptr inbounds nuw (i8, ptr @$str3, i16 6), ptr %94
  call cc 1005 addrspace(1) void @N$PS(ptr %90)
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str14, i16 6))
  call cc 1005 addrspace(1) void @N$PS(ptr nonnull %95)
  call cc 1005 addrspace(1) void @N$PS(ptr nonnull %86)
  call cc 1005 addrspace(1) void @N$PN()
  call cc 1005 addrspace(1) void @N$BDRP(ptr nonnull %95)
  call cc 1005 addrspace(1) void @N$BDRP(ptr %90)
  call cc 1005 addrspace(1) void @N$BDRP(ptr nonnull %86)
  %96 = add i16 %83, -1
  %97 = getelementptr i8, ptr %82, i16 4
  %exitcond3 = icmp eq i16 %96, 0
  br i1 %exitcond3, label %b20, label %81

b20:
  %98 = getelementptr i8, ptr %49, i16 -4
  %99 = load i16, ptr %98
  %100 = icmp eq i16 %99, 0
  br i1 %100, label %b27, label %101

101:
  br label %b25

b25:
  %102 = phi i16 [ %118, %b30 ], [ 0, %101 ]
  %103 = phi ptr [ %117, %b30 ], [ getelementptr inbounds nuw (i8, ptr @$str6, i16 6), %101 ]
  %104 = getelementptr i8, ptr %49, i16 -4
  %105 = load i16, ptr %104
  %106 = icmp ult i16 %102, %105
  br i1 %106, label %b31, label %b32

b27:
  %107 = phi ptr [ getelementptr inbounds nuw (i8, ptr @$str6, i16 6), %b20 ], [ %117, %b30 ]
  %108 = getelementptr i8, ptr %107, i16 -4
  %109 = load i16, ptr %108
  %110 = icmp eq i16 %109, 0
  br i1 %110, label %b36, label %b35

b28:
  %111 = getelementptr i8, ptr %103, i16 -4
  %112 = load i16, ptr %111
  %113 = call cc 1005 addrspace(1) ptr @N$BGRW(ptr %103, i16 1, i16 4)
  %114 = getelementptr i8, ptr %49, i16 -4
  %115 = load i16, ptr %114
  %116 = icmp ult i16 %102, %115
  br i1 %116, label %b33, label %b34

b30:
  %117 = phi ptr [ %113, %b33 ], [ %103, %b31 ]
  %118 = add nuw i16 %102, 1
  %exitcond2 = icmp eq i16 %99, %118
  br i1 %exitcond2, label %b27, label %b25

b31:
  %119 = shl i16 %102, 2
  %120 = getelementptr i8, ptr %49, i16 %119
  %121 = getelementptr i8, ptr %120, i16 2
  %122 = load i16, ptr %121
  %123 = icmp sgt i16 %122, 1500
  br i1 %123, label %b28, label %b30

b32:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b33:
  %124 = shl i16 %112, 2
  %125 = getelementptr inbounds i8, ptr %113, i16 %124
  %126 = getelementptr i8, ptr %49, i16 %119
  %127 = getelementptr i8, ptr %126, i16 2
  %128 = load i16, ptr %127
  store i16 %128, ptr %125
  %129 = getelementptr i8, ptr %125, i16 2
  store i16 %102, ptr %129
  br label %b30

b34:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b35:
  %130 = getelementptr i8, ptr %107, i16 2
  call cc 1005 addrspace(1) void @N$PU2(i16 %109)
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str15, i16 6))
  %131 = load i16, ptr %130
  %132 = getelementptr i8, ptr %49, i16 -4
  %133 = load i16, ptr %132
  %134 = icmp ult i16 %131, %133
  br i1 %134, label %b40, label %b38

b36:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b38:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b39:
  call cc 1005 addrspace(1) void @N$BDRP(ptr nonnull %49)
  ret i16 0

b40:
  %135 = shl i16 %131, 2
  %136 = getelementptr inbounds i8, ptr %49, i16 %135
  %137 = load ptr, ptr %136
  call cc 1005 addrspace(1) void @N$PS(ptr %137)
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str16, i16 6))
  %138 = load i16, ptr %107
  call cc 1005 addrspace(1) void @N$PI2(i16 %138)
  call cc 1005 addrspace(1) void @N$PN()
  call cc 1005 addrspace(1) void @N$BDRP(ptr nonnull %107)
  call cc 1005 addrspace(1) void @N$BDRP(ptr null)
  %139 = getelementptr i8, ptr %49, i16 -4
  %140 = load i16, ptr %139
  %141 = icmp eq i16 %140, 0
  br i1 %141, label %b39, label %142

142:
  br label %b43

b43:
  %143 = phi ptr [ %49, %142 ], [ %147, %b43 ]
  %144 = phi i16 [ %140, %142 ], [ %146, %b43 ]
  %145 = load ptr, ptr %143
  call cc 1005 addrspace(1) void @N$BDRP(ptr %145)
  %146 = add i16 %144, -1
  %147 = getelementptr i8, ptr %143, i16 4
  %exitcond1 = icmp eq i16 %146, 0
  br i1 %exitcond1, label %b39, label %b43
}

declare cc 1005 ptr @N$VCPY(ptr addrspace(1)) addrspace(1)

declare cc 1005 void @N$PBEG() addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PS(ptr) addrspace(1)

declare cc 1005 void @N$PI2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 ptr @N$PEND() addrspace(1)

declare cc 1005 void @N$EBND() addrspace(1) noreturn memory(inaccessiblemem: readwrite)

declare void @llvm.memcpy.p1.p0.i16(ptr addrspace(1) noalias nocapture writeonly, ptr noalias nocapture readonly, i16, i1 immarg) nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)

declare cc 1005 ptr @N$BGRW(ptr, i16, i16) addrspace(1)

declare cc 1005 void @N$BDRP(ptr) addrspace(1)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PU2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{i8 0, i8 2}
!6 = !{i8 0, i8 2}
