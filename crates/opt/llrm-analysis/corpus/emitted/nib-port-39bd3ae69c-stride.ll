target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

define internal cc 1005 i32 @total(ptr addrspace(1) noalias readonly dereferenceable(8) nocapture %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca i16
  %2 = alloca i32
  store i32 0, ptr %2, !tbaa !2
  %3 = load i16, ptr addrspace(1) %0
  store i16 0, ptr %1, !tbaa !2
  br label %b2

b2:
  %4 = load i16, ptr %1, !tbaa !2
  %5 = icmp ult i16 %4, %3
  br i1 %5, label %b3, label %b5

b3:
  %6 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %7 = load ptr addrspace(1), ptr addrspace(1) %6
  %8 = mul i16 %4, 10
  %9 = getelementptr inbounds i8, ptr addrspace(1) %7, i16 %8
  %10 = load i32, ptr %2, !tbaa !2
  %11 = getelementptr i8, ptr addrspace(1) %9, i16 2
  %12 = load i32, ptr addrspace(1) %11
  %13 = add i32 %10, %12
  store i32 %13, ptr %2, !tbaa !2
  br label %b4

b4:
  %14 = load i16, ptr %1, !tbaa !2
  %15 = add i16 %14, 1
  store i16 %15, ptr %1, !tbaa !2
  br label %b2

b5:
  %16 = load i32, ptr %2, !tbaa !2
  ret i32 %16
}

define internal cc 1005 i32 @update(ptr addrspace(1) noalias readonly dereferenceable(8) nocapture %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca [8 x i8]
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca [50 x i8]
  store i16 5, ptr %3, !tbaa !2
  store i16 5, ptr %4, !tbaa !2
  %6 = load i16, ptr addrspace(1) %0, !range !5
  %7 = icmp ult i16 0, %6
  br i1 %7, label %b2, label %b3

b2:
  %8 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %9 = load ptr addrspace(1), ptr addrspace(1) %8
  %10 = getelementptr inbounds i8, ptr addrspace(1) %9, i16 0
  %11 = load i32, ptr addrspace(1) %10
  %12 = load i16, ptr addrspace(1) %0, !range !6
  %13 = icmp ult i16 1, %12
  br i1 %13, label %b4, label %b5

b3:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b4:
  %14 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %15 = load ptr addrspace(1), ptr addrspace(1) %14
  %16 = getelementptr inbounds i8, ptr addrspace(1) %15, i16 4
  %17 = load i32, ptr addrspace(1) %16
  %18 = getelementptr inbounds [10 x i8], ptr %5, i16 0
  store i16 0, ptr %18, !tbaa !2
  %19 = getelementptr inbounds [10 x i8], ptr %5, i16 0
  %20 = getelementptr inbounds i8, ptr %19, i16 2
  store i32 %11, ptr %20, !tbaa !2
  %21 = getelementptr inbounds [10 x i8], ptr %5, i16 0
  %22 = getelementptr inbounds i8, ptr %21, i16 6
  store i32 %17, ptr %22, !tbaa !2
  %23 = load i16, ptr addrspace(1) %0, !range !7
  %24 = icmp ult i16 1, %23
  br i1 %24, label %b6, label %b7

b5:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b6:
  %25 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %26 = load ptr addrspace(1), ptr addrspace(1) %25
  %27 = getelementptr inbounds i8, ptr addrspace(1) %26, i16 4
  %28 = load i32, ptr addrspace(1) %27
  %29 = load i16, ptr addrspace(1) %0, !range !8
  %30 = icmp ult i16 2, %29
  br i1 %30, label %b8, label %b9

b7:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b8:
  %31 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %32 = load ptr addrspace(1), ptr addrspace(1) %31
  %33 = getelementptr inbounds i8, ptr addrspace(1) %32, i16 8
  %34 = load i32, ptr addrspace(1) %33
  %35 = getelementptr inbounds [10 x i8], ptr %5, i16 1
  store i16 0, ptr %35, !tbaa !2
  %36 = getelementptr inbounds [10 x i8], ptr %5, i16 1
  %37 = getelementptr inbounds i8, ptr %36, i16 2
  store i32 %28, ptr %37, !tbaa !2
  %38 = getelementptr inbounds [10 x i8], ptr %5, i16 1
  %39 = getelementptr inbounds i8, ptr %38, i16 6
  store i32 %34, ptr %39, !tbaa !2
  %40 = load i16, ptr addrspace(1) %0, !range !9
  %41 = icmp ult i16 2, %40
  br i1 %41, label %b10, label %b11

b9:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b10:
  %42 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %43 = load ptr addrspace(1), ptr addrspace(1) %42
  %44 = getelementptr inbounds i8, ptr addrspace(1) %43, i16 8
  %45 = load i32, ptr addrspace(1) %44
  %46 = load i16, ptr addrspace(1) %0, !range !10
  %47 = icmp ult i16 3, %46
  br i1 %47, label %b12, label %b13

b11:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b12:
  %48 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %49 = load ptr addrspace(1), ptr addrspace(1) %48
  %50 = getelementptr inbounds i8, ptr addrspace(1) %49, i16 12
  %51 = load i32, ptr addrspace(1) %50
  %52 = getelementptr inbounds [10 x i8], ptr %5, i16 2
  store i16 0, ptr %52, !tbaa !2
  %53 = getelementptr inbounds [10 x i8], ptr %5, i16 2
  %54 = getelementptr inbounds i8, ptr %53, i16 2
  store i32 %45, ptr %54, !tbaa !2
  %55 = getelementptr inbounds [10 x i8], ptr %5, i16 2
  %56 = getelementptr inbounds i8, ptr %55, i16 6
  store i32 %51, ptr %56, !tbaa !2
  %57 = load i16, ptr addrspace(1) %0, !range !11
  %58 = icmp ult i16 3, %57
  br i1 %58, label %b14, label %b15

b13:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b14:
  %59 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %60 = load ptr addrspace(1), ptr addrspace(1) %59
  %61 = getelementptr inbounds i8, ptr addrspace(1) %60, i16 12
  %62 = load i32, ptr addrspace(1) %61
  %63 = load i16, ptr addrspace(1) %0, !range !12
  %64 = icmp ult i16 4, %63
  br i1 %64, label %b16, label %b17

b15:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b16:
  %65 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %66 = load ptr addrspace(1), ptr addrspace(1) %65
  %67 = getelementptr inbounds i8, ptr addrspace(1) %66, i16 16
  %68 = load i32, ptr addrspace(1) %67
  %69 = getelementptr inbounds [10 x i8], ptr %5, i16 3
  store i16 0, ptr %69, !tbaa !2
  %70 = getelementptr inbounds [10 x i8], ptr %5, i16 3
  %71 = getelementptr inbounds i8, ptr %70, i16 2
  store i32 %62, ptr %71, !tbaa !2
  %72 = getelementptr inbounds [10 x i8], ptr %5, i16 3
  %73 = getelementptr inbounds i8, ptr %72, i16 6
  store i32 %68, ptr %73, !tbaa !2
  %74 = load i16, ptr addrspace(1) %0, !range !13
  %75 = icmp ult i16 4, %74
  br i1 %75, label %b18, label %b19

b17:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b18:
  %76 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %77 = load ptr addrspace(1), ptr addrspace(1) %76
  %78 = getelementptr inbounds i8, ptr addrspace(1) %77, i16 16
  %79 = load i32, ptr addrspace(1) %78
  %80 = load i16, ptr addrspace(1) %0, !range !14
  %81 = icmp ult i16 5, %80
  br i1 %81, label %b20, label %b21

b19:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b20:
  %82 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %83 = load ptr addrspace(1), ptr addrspace(1) %82
  %84 = getelementptr inbounds i8, ptr addrspace(1) %83, i16 20
  %85 = load i32, ptr addrspace(1) %84
  %86 = getelementptr inbounds [10 x i8], ptr %5, i16 4
  store i16 0, ptr %86, !tbaa !2
  %87 = getelementptr inbounds [10 x i8], ptr %5, i16 4
  %88 = getelementptr inbounds i8, ptr %87, i16 2
  store i32 %79, ptr %88, !tbaa !2
  %89 = getelementptr inbounds [10 x i8], ptr %5, i16 4
  %90 = getelementptr inbounds i8, ptr %89, i16 6
  store i32 %85, ptr %90, !tbaa !2
  store i16 0, ptr %2, !tbaa !2
  br label %b22

b21:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b22:
  %91 = load i16, ptr %2, !tbaa !2
  %92 = icmp ult i16 %91, 5
  br i1 %92, label %b23, label %b25

b23:
  %93 = sub i16 %91, 0
  %94 = getelementptr inbounds [10 x i8], ptr %5, i16 %93
  %95 = getelementptr inbounds i8, ptr %94, i16 2
  %96 = load i32, ptr %95, !tbaa !2
  %97 = sub i16 %91, 0
  %98 = getelementptr inbounds [10 x i8], ptr %5, i16 %97
  %99 = getelementptr inbounds i8, ptr %98, i16 6
  %100 = load i32, ptr %99, !tbaa !2
  %101 = add i32 %96, %100
  %102 = sub i16 %91, 0
  %103 = getelementptr inbounds [10 x i8], ptr %5, i16 %102
  %104 = getelementptr inbounds i8, ptr %103, i16 2
  store i32 %101, ptr %104, !tbaa !2
  br label %b24

b24:
  %105 = load i16, ptr %2, !tbaa !2
  %106 = add i16 %105, 1
  store i16 %106, ptr %2, !tbaa !2
  br label %b22

b25:
  %107 = addrspacecast ptr %5 to ptr addrspace(1)
  store i16 5, ptr %1, !tbaa !2
  %108 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 5, ptr %108, !tbaa !2
  %109 = getelementptr inbounds i8, ptr %1, i16 4
  store ptr addrspace(1) %107, ptr %109, !tbaa !2
  %110 = addrspacecast ptr %1 to ptr addrspace(1)
  %111 = call cc 1005 addrspace(1) i32 @total(ptr addrspace(1) %110)
  ret i32 %111
}

define internal cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca [8 x i8]
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca [24 x i8]
  store i16 6, ptr %1, !tbaa !2
  store i16 6, ptr %2, !tbaa !2
  %4 = getelementptr inbounds i32, ptr %3, i16 0
  store i32 1, ptr %4, !tbaa !2
  %5 = getelementptr inbounds i32, ptr %3, i16 1
  store i32 2, ptr %5, !tbaa !2
  %6 = getelementptr inbounds i32, ptr %3, i16 2
  store i32 3, ptr %6, !tbaa !2
  %7 = getelementptr inbounds i32, ptr %3, i16 3
  store i32 4, ptr %7, !tbaa !2
  %8 = getelementptr inbounds i32, ptr %3, i16 4
  store i32 5, ptr %8, !tbaa !2
  %9 = getelementptr inbounds i32, ptr %3, i16 5
  store i32 6, ptr %9, !tbaa !2
  %10 = addrspacecast ptr %3 to ptr addrspace(1)
  store i16 6, ptr %0, !tbaa !2
  %11 = getelementptr inbounds i8, ptr %0, i16 2
  store i16 6, ptr %11, !tbaa !2
  %12 = getelementptr inbounds i8, ptr %0, i16 4
  store ptr addrspace(1) %10, ptr %12, !tbaa !2
  %13 = addrspacecast ptr %0 to ptr addrspace(1)
  %14 = call cc 1005 addrspace(1) i32 @update(ptr addrspace(1) %13)
  ret i16 0
}

declare cc 1005 void @N$EBND() addrspace(1) noreturn memory(inaccessiblemem: readwrite)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{i16 0, i16 16384}
!6 = !{i16 0, i16 16384}
!7 = !{i16 0, i16 16384}
!8 = !{i16 0, i16 16384}
!9 = !{i16 0, i16 16384}
!10 = !{i16 0, i16 16384}
!11 = !{i16 0, i16 16384}
!12 = !{i16 0, i16 16384}
!13 = !{i16 0, i16 16384}
!14 = !{i16 0, i16 16384}
