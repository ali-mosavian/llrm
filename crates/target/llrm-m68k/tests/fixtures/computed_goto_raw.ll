; ModuleID = 'tests/run/c/computed_goto.c'
source_filename = "tests/run/c/computed_goto.c"
target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
target triple = "m68k"

@run.table = internal constant [7 x ptr] [ptr blockaddress(@run, %13), ptr blockaddress(@run, %24), ptr blockaddress(@run, %39), ptr blockaddress(@run, %54), ptr blockaddress(@run, %77), ptr blockaddress(@run, %67), ptr blockaddress(@run, %97)], align 2
@__const.main.program = private unnamed_addr constant [11 x i32] [i32 0, i32 3, i32 0, i32 4, i32 1, i32 0, i32 6, i32 5, i32 4, i32 7, i32 6], align 2
@__const.main.second = private unnamed_addr constant [8 x i32] [i32 0, i32 5, i32 3, i32 2, i32 0, i32 2, i32 1, i32 6], align 2

; Function Attrs: noinline nounwind
define dso_local i32 @run(ptr noundef %0) #0 {
  %2 = alloca ptr, align 2
  %3 = alloca [16 x i32], align 2
  %4 = alloca ptr, align 2
  %5 = alloca ptr, align 2
  store ptr %0, ptr %2, align 2
  %6 = getelementptr inbounds [16 x i32], ptr %3, i32 0, i32 0
  store ptr %6, ptr %4, align 2
  %7 = load ptr, ptr %2, align 2
  store ptr %7, ptr %5, align 2
  %8 = load ptr, ptr %5, align 2
  %9 = getelementptr inbounds nuw i32, ptr %8, i32 1
  store ptr %9, ptr %5, align 2
  %10 = load i32, ptr %8, align 2
  %11 = getelementptr inbounds [7 x ptr], ptr @run.table, i32 0, i32 %10
  %12 = load ptr, ptr %11, align 2
  br label %101

13:                                               ; preds = %101
  %14 = load ptr, ptr %5, align 2
  %15 = getelementptr inbounds nuw i32, ptr %14, i32 1
  store ptr %15, ptr %5, align 2
  %16 = load i32, ptr %14, align 2
  %17 = load ptr, ptr %4, align 2
  %18 = getelementptr inbounds nuw i32, ptr %17, i32 1
  store ptr %18, ptr %4, align 2
  store i32 %16, ptr %17, align 2
  %19 = load ptr, ptr %5, align 2
  %20 = getelementptr inbounds nuw i32, ptr %19, i32 1
  store ptr %20, ptr %5, align 2
  %21 = load i32, ptr %19, align 2
  %22 = getelementptr inbounds [7 x ptr], ptr @run.table, i32 0, i32 %21
  %23 = load ptr, ptr %22, align 2
  br label %101

24:                                               ; preds = %101
  %25 = load ptr, ptr %4, align 2
  %26 = getelementptr inbounds i32, ptr %25, i32 -1
  store ptr %26, ptr %4, align 2
  %27 = load ptr, ptr %4, align 2
  %28 = getelementptr inbounds i32, ptr %27, i32 0
  %29 = load i32, ptr %28, align 2
  %30 = load ptr, ptr %4, align 2
  %31 = getelementptr inbounds i32, ptr %30, i32 -1
  %32 = load i32, ptr %31, align 2
  %33 = add nsw i32 %32, %29
  store i32 %33, ptr %31, align 2
  %34 = load ptr, ptr %5, align 2
  %35 = getelementptr inbounds nuw i32, ptr %34, i32 1
  store ptr %35, ptr %5, align 2
  %36 = load i32, ptr %34, align 2
  %37 = getelementptr inbounds [7 x ptr], ptr @run.table, i32 0, i32 %36
  %38 = load ptr, ptr %37, align 2
  br label %101

39:                                               ; preds = %101
  %40 = load ptr, ptr %4, align 2
  %41 = getelementptr inbounds i32, ptr %40, i32 -1
  store ptr %41, ptr %4, align 2
  %42 = load ptr, ptr %4, align 2
  %43 = getelementptr inbounds i32, ptr %42, i32 0
  %44 = load i32, ptr %43, align 2
  %45 = load ptr, ptr %4, align 2
  %46 = getelementptr inbounds i32, ptr %45, i32 -1
  %47 = load i32, ptr %46, align 2
  %48 = mul nsw i32 %47, %44
  store i32 %48, ptr %46, align 2
  %49 = load ptr, ptr %5, align 2
  %50 = getelementptr inbounds nuw i32, ptr %49, i32 1
  store ptr %50, ptr %5, align 2
  %51 = load i32, ptr %49, align 2
  %52 = getelementptr inbounds [7 x ptr], ptr @run.table, i32 0, i32 %51
  %53 = load ptr, ptr %52, align 2
  br label %101

54:                                               ; preds = %101
  %55 = load ptr, ptr %4, align 2
  %56 = getelementptr inbounds i32, ptr %55, i32 -1
  %57 = load i32, ptr %56, align 2
  %58 = load ptr, ptr %4, align 2
  %59 = getelementptr inbounds i32, ptr %58, i32 0
  store i32 %57, ptr %59, align 2
  %60 = load ptr, ptr %4, align 2
  %61 = getelementptr inbounds nuw i32, ptr %60, i32 1
  store ptr %61, ptr %4, align 2
  %62 = load ptr, ptr %5, align 2
  %63 = getelementptr inbounds nuw i32, ptr %62, i32 1
  store ptr %63, ptr %5, align 2
  %64 = load i32, ptr %62, align 2
  %65 = getelementptr inbounds [7 x ptr], ptr @run.table, i32 0, i32 %64
  %66 = load ptr, ptr %65, align 2
  br label %101

67:                                               ; preds = %101
  %68 = load ptr, ptr %4, align 2
  %69 = getelementptr inbounds i32, ptr %68, i32 -1
  %70 = load i32, ptr %69, align 2
  %71 = sub nsw i32 %70, 1
  store i32 %71, ptr %69, align 2
  %72 = load ptr, ptr %5, align 2
  %73 = getelementptr inbounds nuw i32, ptr %72, i32 1
  store ptr %73, ptr %5, align 2
  %74 = load i32, ptr %72, align 2
  %75 = getelementptr inbounds [7 x ptr], ptr @run.table, i32 0, i32 %74
  %76 = load ptr, ptr %75, align 2
  br label %101

77:                                               ; preds = %101
  %78 = load ptr, ptr %4, align 2
  %79 = getelementptr inbounds i32, ptr %78, i32 -1
  %80 = load i32, ptr %79, align 2
  %81 = icmp ne i32 %80, 0
  br i1 %81, label %82, label %87

82:                                               ; preds = %77
  %83 = load ptr, ptr %2, align 2
  %84 = load ptr, ptr %5, align 2
  %85 = load i32, ptr %84, align 2
  %86 = getelementptr inbounds i32, ptr %83, i32 %85
  br label %90

87:                                               ; preds = %77
  %88 = load ptr, ptr %5, align 2
  %89 = getelementptr inbounds i32, ptr %88, i32 1
  br label %90

90:                                               ; preds = %87, %82
  %91 = phi ptr [ %86, %82 ], [ %89, %87 ]
  store ptr %91, ptr %5, align 2
  %92 = load ptr, ptr %5, align 2
  %93 = getelementptr inbounds nuw i32, ptr %92, i32 1
  store ptr %93, ptr %5, align 2
  %94 = load i32, ptr %92, align 2
  %95 = getelementptr inbounds [7 x ptr], ptr @run.table, i32 0, i32 %94
  %96 = load ptr, ptr %95, align 2
  br label %101

97:                                               ; preds = %101
  %98 = load ptr, ptr %4, align 2
  %99 = getelementptr inbounds i32, ptr %98, i32 -1
  %100 = load i32, ptr %99, align 2
  ret i32 %100

101:                                              ; preds = %90, %67, %54, %39, %24, %13, %1
  %102 = phi ptr [ %12, %1 ], [ %23, %13 ], [ %38, %24 ], [ %53, %39 ], [ %66, %54 ], [ %76, %67 ], [ %96, %90 ]
  indirectbr ptr %102, [label %13, label %24, label %39, label %54, label %77, label %67, label %97]
}

; Function Attrs: noinline nounwind
define dso_local i32 @jump_to(i32 noundef %0, i32 noundef %1) #0 {
  %3 = alloca i32, align 2
  %4 = alloca i32, align 2
  %5 = alloca i32, align 2
  %6 = alloca ptr, align 2
  store i32 %0, ptr %4, align 2
  store i32 %1, ptr %5, align 2
  %7 = load i32, ptr %4, align 2
  %8 = icmp ne i32 %7, 0
  %9 = zext i1 %8 to i64
  %10 = select i1 %8, ptr blockaddress(@jump_to, %12), ptr blockaddress(@jump_to, %15)
  store ptr %10, ptr %6, align 2
  %11 = load ptr, ptr %6, align 2
  br label %20

12:                                               ; preds = %20
  %13 = load i32, ptr %5, align 2
  %14 = add nsw i32 %13, 1
  store i32 %14, ptr %3, align 2
  br label %18

15:                                               ; preds = %20
  %16 = load i32, ptr %5, align 2
  %17 = mul nsw i32 %16, 2
  store i32 %17, ptr %3, align 2
  br label %18

18:                                               ; preds = %15, %12
  %19 = load i32, ptr %3, align 2
  ret i32 %19

20:                                               ; preds = %2
  %21 = phi ptr [ %11, %2 ]
  indirectbr ptr %21, [label %12, label %15]
}

; Function Attrs: noinline nounwind
define dso_local i32 @main() #0 {
  %1 = alloca i32, align 2
  %2 = alloca [11 x i32], align 2
  %3 = alloca [8 x i32], align 2
  store i32 0, ptr %1, align 2
  call void @llvm.memcpy.p0.p0.i32(ptr align 2 %2, ptr align 2 @__const.main.program, i32 44, i1 false)
  %4 = getelementptr inbounds [11 x i32], ptr %2, i32 0, i32 0
  %5 = call i32 @run(ptr noundef %4)
  call void @report(i32 noundef %5)
  call void @llvm.memcpy.p0.p0.i32(ptr align 2 %3, ptr align 2 @__const.main.second, i32 32, i1 false)
  %6 = getelementptr inbounds [8 x i32], ptr %3, i32 0, i32 0
  %7 = call i32 @run(ptr noundef %6)
  call void @report(i32 noundef %7)
  %8 = call i32 @jump_to(i32 noundef 1, i32 noundef 10)
  call void @report(i32 noundef %8)
  %9 = call i32 @jump_to(i32 noundef 0, i32 noundef 10)
  call void @report(i32 noundef %9)
  ret i32 0
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.memcpy.p0.p0.i32(ptr noalias writeonly captures(none), ptr noalias readonly captures(none), i32, i1 immarg) #1

declare dso_local void @report(i32 noundef) #2

attributes #0 = { noinline nounwind "frame-pointer"="all" "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { "frame-pointer"="all" "no-trapping-math"="true" "stack-protector-buffer-size"="8" }

!llvm.module.flags = !{!0}
!llvm.ident = !{!1}

!0 = !{i32 7, !"frame-pointer", i32 2}
!1 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
