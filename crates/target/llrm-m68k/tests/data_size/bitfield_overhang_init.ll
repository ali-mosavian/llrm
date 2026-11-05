target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"

%struct.S = type { i32 }

@g = global { i8, i8, i8, i8 } { i8 90, i8 18, i8 52, i8 86 }, align 2
@table = global [2 x { i8, i8, i8, i8 }] [{ i8, i8, i8, i8 } { i8 1, i8 0, i8 0, i8 2 }, { i8, i8, i8, i8 } { i8 127, i8 -1, i8 -1, i8 -3 }], align 2

define i32 @main() nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000" {
  %1 = load i32, ptr @g, align 2
  %2 = ashr i32 %1, 24
  tail call void @report(i32 noundef %2) nobuiltin "no-builtins"
  %3 = load i32, ptr @g, align 2
  %4 = shl i32 %3, 8
  %5 = ashr exact i32 %4, 8
  tail call void @report(i32 noundef %5) nobuiltin "no-builtins"
  %6 = load i32, ptr @table, align 2
  %7 = ashr i32 %6, 24
  tail call void @report(i32 noundef %7) nobuiltin "no-builtins"
  %8 = load i32, ptr @table, align 2
  %9 = shl i32 %8, 8
  %10 = ashr exact i32 %9, 8
  tail call void @report(i32 noundef %10) nobuiltin "no-builtins"
  %11 = load i32, ptr getelementptr inbounds nuw (i8, ptr @table, i32 4), align 2
  %12 = ashr i32 %11, 24
  tail call void @report(i32 noundef %12) nobuiltin "no-builtins"
  %13 = load i32, ptr getelementptr inbounds nuw (i8, ptr @table, i32 4), align 2
  %14 = shl i32 %13, 8
  %15 = ashr exact i32 %14, 8
  tail call void @report(i32 noundef %15) nobuiltin "no-builtins"
  tail call void @report(i32 noundef 3) nobuiltin "no-builtins"
  tail call void @report(i32 noundef 66051) nobuiltin "no-builtins"
  tail call void @report(i32 noundef 4) nobuiltin "no-builtins"
  ret i32 0
}

declare void @llvm.lifetime.start.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare void @llvm.memcpy.p0.p0.i32(ptr noalias nocapture writeonly, ptr noalias nocapture readonly, i32, i1 immarg) nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)

declare void @report(i32 noundef) "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000"

declare void @llvm.lifetime.end.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

!llvm.module.flags = !{!0, !1}
!llvm.ident = !{!2}
!llvm.errno.tbaa = !{!3}

!0 = !{i32 8, !"PIC Level", i32 2}
!1 = !{i32 7, !"PIE Level", i32 2}
!2 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!3 = !{!4, !5, i64 0}
!4 = !{!"__libc_errno", !5, i64 0}
!5 = !{!"int", !6, i64 0}
!6 = !{!"omnipotent char", !7, i64 0}
!7 = !{!"Simple C/C++ TBAA"}
