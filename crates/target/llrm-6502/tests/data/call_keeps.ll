; ModuleID = 'tests/run/c/call_keeps.c'
source_filename = "tests/run/c/call_keeps.c"
target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8"

; Function Attrs: nounwind optsize
define dso_local noundef i16 @main() local_unnamed_addr #0 {
  %1 = tail call fastcc i32 @hold(i32 noundef 1) #3
  tail call void @report(i32 noundef %1) #4
  %2 = tail call fastcc i32 @hold(i32 noundef 77) #3
  tail call void @report(i32 noundef %2) #4
  %3 = tail call fastcc i32 @hold(i32 noundef 65535) #3
  tail call void @report(i32 noundef %3) #4
  ret i16 0
}

; Function Attrs: optsize
declare dso_local void @report(i32 noundef) local_unnamed_addr #1

; Function Attrs: mustprogress nofree norecurse nosync nounwind optsize willreturn memory(none)
define internal fastcc i32 @hold(i32 noundef range(i32 1, 65536) %0) unnamed_addr #2 {
  %2 = add nuw nsw i32 %0, 305419896
  %3 = xor i32 %0, -1698898192
  %4 = sub nuw nsw i32 -19088744, %0
  %5 = mul nuw nsw i32 %0, 3
  %6 = add nuw nsw i32 %5, -2147483647
  %7 = tail call fastcc i32 @churn(i32 noundef %2, i32 noundef %3, i32 noundef %4, i32 noundef %6) #3
  %8 = add i32 %7, %2
  %9 = tail call fastcc i32 @churn(i32 noundef %3, i32 noundef %4, i32 noundef %6, i32 noundef %8) #3
  %10 = xor i32 %2, %3
  %11 = xor i32 %6, %4
  %12 = add nsw i32 %11, %10
  %13 = add i32 %12, %7
  %14 = add i32 %13, %9
  ret i32 %14
}

; Function Attrs: mustprogress nofree norecurse nosync nounwind optsize willreturn memory(none)
define internal fastcc i32 @churn(i32 noundef range(i32 -1698955264, 305485432) %0, i32 noundef range(i32 -1698955264, -19088744) %1, i32 noundef range(i32 -2147483644, -19088744) %2, i32 noundef %3) unnamed_addr #2 {
  %5 = mul i32 %0, 3
  %6 = add i32 %5, %1
  %7 = mul i32 %1, 5
  %8 = add i32 %7, %2
  %9 = mul i32 %2, 7
  %10 = add i32 %9, %3
  %11 = mul i32 %3, 11
  %12 = add i32 %11, %0
  %13 = xor i32 %3, %0
  %14 = xor i32 %2, %1
  %15 = shl i32 %6, 3
  %16 = lshr i32 %8, 2
  %17 = xor i32 %16, %15
  %18 = xor i32 %17, %13
  %19 = shl i32 %8, 5
  %20 = lshr i32 %10, 3
  %21 = xor i32 %19, %20
  %22 = xor i32 %21, %14
  %23 = shl i32 %10, 7
  %24 = lshr i32 %12, 5
  %25 = xor i32 %24, %23
  %26 = xor i32 %25, %18
  %27 = shl i32 %12, 9
  %28 = lshr i32 %18, 7
  %29 = xor i32 %28, %27
  %30 = xor i32 %29, %22
  %31 = add i32 %13, %14
  %32 = add i32 %31, %18
  %33 = add i32 %32, %22
  %34 = add i32 %33, %26
  %35 = add i32 %34, %30
  ret i32 %35
}

attributes #0 = { nounwind optsize "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #1 = { optsize "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #2 = { mustprogress nofree norecurse nosync nounwind optsize willreturn memory(none) "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #3 = { optsize }
attributes #4 = { nounwind optsize }

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
