; ModuleID = 's.c'
source_filename = "s.c"
target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32-unknown-unknown"

%struct.s3 = type { i8, i8, i8 }
%struct.s12 = type { i32, i32, i32 }
%struct.s100 = type { [25 x i32] }
%struct.m = type { double, i32 }
%struct.s1 = type { i8 }

; Function Attrs: noinline nounwind
define i32 @g(i8 %0, ptr byval(%struct.s3) align 1 %1, ptr byval(%struct.s12) align 4 %2, ptr byval(%struct.s100) align 4 %3, ptr byval(%struct.m) align 8 %4) {
  %6 = alloca %struct.s1, align 1
  %7 = alloca %struct.s12, align 4
  %8 = alloca %struct.m, align 8
  %9 = alloca %struct.s100, align 4
  %10 = getelementptr inbounds nuw %struct.s1, ptr %6, i32 0, i32 0
  store i8 %0, ptr %10, align 1
  call void @f12(ptr sret(%struct.s12) align 4 %7, ptr byval(%struct.s12) align 4 %2, ptr byval(%struct.s12) align 4 %2)
  call void @fm(ptr sret(%struct.m) align 8 %8, ptr byval(%struct.m) align 8 %4)
  call void @big(ptr sret(%struct.s100) align 4 %9, ptr byval(%struct.s100) align 4 %3, i32 3)
  %11 = getelementptr inbounds nuw %struct.s1, ptr %6, i32 0, i32 0
  %12 = load i8, ptr %11, align 1
  %13 = call i32 @f1(i8 %12)
  %14 = call i32 @f3(ptr byval(%struct.s3) align 1 %1)
  %15 = add nsw i32 %13, %14
  %16 = getelementptr inbounds nuw %struct.s12, ptr %7, i32 0, i32 1
  %17 = load i32, ptr %16, align 4
  %18 = add nsw i32 %15, %17
  %19 = getelementptr inbounds nuw %struct.s100, ptr %9, i32 0, i32 0
  %20 = getelementptr inbounds [25 x i32], ptr %19, i32 0, i32 2
  %21 = load i32, ptr %20, align 4
  %22 = call i32 @f100(ptr byval(%struct.s100) align 4 %3, i32 %21)
  %23 = add nsw i32 %18, %22
  %24 = getelementptr inbounds nuw %struct.m, ptr %8, i32 0, i32 0
  %25 = load double, ptr %24, align 8
  %26 = fptosi double %25 to i32
  %27 = add nsw i32 %23, %26
  ret i32 %27
}

declare void @f12(ptr sret(%struct.s12) align 4, ptr byval(%struct.s12) align 4, ptr byval(%struct.s12) align 4)

declare void @fm(ptr sret(%struct.m) align 8, ptr byval(%struct.m) align 8)

declare void @big(ptr sret(%struct.s100) align 4, ptr byval(%struct.s100) align 4, i32)

declare i32 @f1(i8)

declare i32 @f3(ptr byval(%struct.s3) align 1)

declare i32 @f100(ptr byval(%struct.s100) align 4, i32)

; Function Attrs: noinline nounwind
define void @mk(ptr noalias sret(%struct.s12) align 4 %0, i32 %1) {
  %3 = alloca i32, align 4
  store i32 %1, ptr %3, align 4
  %4 = getelementptr inbounds nuw %struct.s12, ptr %0, i32 0, i32 0
  %5 = load i32, ptr %3, align 4
  store i32 %5, ptr %4, align 4
  %6 = getelementptr inbounds nuw %struct.s12, ptr %0, i32 0, i32 1
  %7 = load i32, ptr %3, align 4
  %8 = add nsw i32 %7, 1
  store i32 %8, ptr %6, align 4
  %9 = getelementptr inbounds nuw %struct.s12, ptr %0, i32 0, i32 2
  %10 = load i32, ptr %3, align 4
  %11 = add nsw i32 %10, 2
  store i32 %11, ptr %9, align 4
  ret void
}

; Function Attrs: noinline nounwind
define i32 @takes(ptr byval(%struct.s100) align 4 %0) {
  %2 = getelementptr inbounds nuw %struct.s100, ptr %0, i32 0, i32 0
  %3 = getelementptr inbounds [25 x i32], ptr %2, i32 0, i32 24
  %4 = load i32, ptr %3, align 4
  ret i32 %4
}

; Function Attrs: noinline nounwind
define void @pass(ptr noalias sret(%struct.s100) align 4 %0, ptr byval(%struct.s100) align 4 %1) {
  %3 = getelementptr inbounds nuw %struct.s100, ptr %1, i32 0, i32 0
  %4 = getelementptr inbounds [25 x i32], ptr %3, i32 0, i32 1
  store i32 5, ptr %4, align 4
  call void @llvm.memcpy.p0.p0.i32(ptr align 4 %0, ptr align 4 %1, i32 100, i1 false)
  ret void
}

; Function Attrs: nocallback nofree nounwind willreturn memory(argmem: readwrite)
declare void @llvm.memcpy.p0.p0.i32(ptr noalias nocapture writeonly, ptr noalias nocapture readonly, i32, i1 immarg)



