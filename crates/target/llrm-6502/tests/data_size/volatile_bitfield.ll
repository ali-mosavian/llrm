target triple = "msp430"

%union.U = type { i32 }
%struct.S = type { i32 }

@u = dso_local global %union.U zeroinitializer, align 1
@v = dso_local global %struct.S zeroinitializer, align 1
@d = dso_local global i16 0, align 2

; Function Attrs: nounwind optsize
define dso_local i16 @main() #0 {
  %1 = alloca i16, align 2
  store i16 0, ptr %1, align 2
  store volatile i32 67305985, ptr @u, align 1, !tbaa !7
  %2 = load volatile i32, ptr @v, align 1
  %3 = and i32 %2, -8
  %4 = or i32 %3, 5
  store volatile i32 %4, ptr @v, align 1
  %5 = load volatile i32, ptr @v, align 1
  %6 = and i32 %5, -262137
  %7 = or i32 %6, 98760
  store volatile i32 %7, ptr @v, align 1
  %8 = load volatile i32, ptr @v, align 1
  %9 = and i32 %8, 262143
  %10 = or i32 %9, 84148224
  store volatile i32 %10, ptr @v, align 1
  %11 = load volatile i32, ptr @u, align 1
  %12 = lshr i32 %11, 3
  %13 = and i32 %12, 32767
  %14 = trunc i32 %13 to i16
  store i16 %14, ptr @d, align 2, !tbaa !8
  %15 = load i16, ptr @d, align 2, !tbaa !8
  %16 = load volatile i32, ptr @u, align 1
  %17 = lshr i32 %16, 3
  %18 = and i32 %17, 32767
  %19 = trunc i32 %18 to i16
  %20 = icmp ne i16 %15, %19
  %21 = zext i1 %20 to i16
  %22 = sext i16 %21 to i32
  call void @report(i32 noundef %22) #2
  %23 = load i16, ptr @d, align 2, !tbaa !8
  %24 = load volatile i32, ptr @u, align 1
  %25 = lshr i32 %24, 3
  %26 = and i32 %25, 32767
  %27 = trunc i32 %26 to i16
  %28 = icmp eq i16 %23, %27
  %29 = zext i1 %28 to i16
  %30 = sext i16 %29 to i32
  call void @report(i32 noundef %30) #2
  %31 = load volatile i32, ptr @v, align 1
  %32 = and i32 %31, 7
  %33 = trunc i32 %32 to i16
  %34 = zext i16 %33 to i32
  %35 = load volatile i32, ptr @v, align 1
  %36 = lshr i32 %35, 3
  %37 = and i32 %36, 32767
  %38 = trunc i32 %37 to i16
  %39 = zext i16 %38 to i32
  %40 = add nsw i32 %34, %39
  %41 = load volatile i32, ptr @v, align 1
  %42 = lshr i32 %41, 18
  %43 = add nsw i32 %40, %42
  call void @report(i32 noundef %43) #2
  %44 = load volatile i32, ptr @v, align 1
  %45 = lshr i32 %44, 3
  %46 = and i32 %45, 32767
  %47 = trunc i32 %46 to i16
  %48 = add nsw i16 %47, 1
  %49 = zext i16 %48 to i32
  %50 = load volatile i32, ptr @v, align 1
  %51 = and i32 %49, 32767
  %52 = shl i32 %51, 3
  %53 = and i32 %50, -262137
  %54 = or i32 %53, %52
  store volatile i32 %54, ptr @v, align 1
  %55 = trunc i32 %51 to i16
  %56 = load volatile i32, ptr @v, align 1
  %57 = lshr i32 %56, 3
  %58 = and i32 %57, 32767
  %59 = trunc i32 %58 to i16
  %60 = zext i16 %59 to i32
  call void @report(i32 noundef %60) #2
  %61 = load volatile i32, ptr @v, align 1
  %62 = lshr i32 %61, 18
  %63 = trunc i32 %62 to i16
  %64 = icmp sgt i16 %63, 300
  br i1 %64, label %65, label %71

65:                                               ; preds = %0
  %66 = load volatile i32, ptr @v, align 1
  %67 = and i32 %66, 7
  %68 = trunc i32 %67 to i16
  %69 = icmp eq i16 %68, 5
  br i1 %69, label %70, label %71

70:                                               ; preds = %65
  call void @report(i32 noundef 1) #2
  br label %72

71:                                               ; preds = %65, %0
  call void @report(i32 noundef 0) #2
  br label %72

72:                                               ; preds = %71, %70
  ret i16 0
}

; Function Attrs: optsize
declare dso_local void @report(i32 noundef) #1

attributes #0 = { nounwind optsize "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #1 = { optsize "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #2 = { optsize }

!llvm.module.flags = !{!0}
!llvm.ident = !{!1}
!llvm.errno.tbaa = !{!2}

!0 = !{i32 1, !"wchar_size", i32 2}
!1 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!2 = !{!3, !4, i64 0}
!3 = !{!"__libc_errno", !4, i64 0}
!4 = !{!"int", !5, i64 0}
!5 = !{!"omnipotent char", !6, i64 0}
!6 = !{!"Simple C/C++ TBAA"}
!7 = !{!5, !5, i64 0}
!8 = !{!4, !4, i64 0}
