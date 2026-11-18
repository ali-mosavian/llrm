; ModuleID = 'shift_until_idioms.ll'
source_filename = "/home/alim/work/personal/llrm/.claude/worktrees/funny-chatelet-66ad45/tests/run/c/shift_until_idioms.c"
target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8"

; Function Attrs: noinline nounwind
define dso_local i32 @until_bit(i32 noundef %0, i32 noundef %1) #0 {
  %3 = alloca i32, align 2
  %4 = alloca i32, align 2
  %5 = alloca i32, align 2
  store i32 %0, ptr %3, align 2, !tbaa !7
  store i32 %1, ptr %4, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %5) #2
  %6 = load i32, ptr %4, align 2, !tbaa !7
  %7 = shl i32 1, %6
  store i32 %7, ptr %5, align 2, !tbaa !7
  br label %8

8:                                                ; preds = %14, %2
  %9 = load i32, ptr %3, align 2, !tbaa !7
  %10 = load i32, ptr %5, align 2, !tbaa !7
  %11 = and i32 %9, %10
  %12 = icmp ne i32 %11, 0
  %13 = xor i1 %12, true
  br i1 %13, label %14, label %17

14:                                               ; preds = %8
  %15 = load i32, ptr %3, align 2, !tbaa !7
  %16 = shl i32 %15, 1
  store i32 %16, ptr %3, align 2, !tbaa !7
  br label %8, !llvm.loop !9

17:                                               ; preds = %8
  %18 = load i32, ptr %3, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %5) #2
  ret i32 %18
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

attributes #0 = { noinline nounwind "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { nounwind }

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
!7 = !{!8, !8, i64 0}
!8 = !{!"long", !5, i64 0}
!9 = distinct !{!9, !10}
!10 = !{!"llvm.loop.mustprogress"}
