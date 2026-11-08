target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"

define void @pack_ref(i32 noundef %0, i32 noundef %1, i32 noundef %2, i32 noundef %3, i32 noundef %4, ptr noundef nocapture nofree %5) nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000" {
  %7 = icmp eq i32 %0, 16
  %8 = icmp eq i32 %0, 13
  %9 = or i1 %7, %8
  %10 = icmp eq i32 %0, 19
  %11 = icmp eq i32 %0, 4
  %12 = zext i1 %11 to i32
  %13 = select i1 %10, i32 3, i32 %12
  %14 = shl i32 %3, %13
  %15 = add i32 %14, 7
  %16 = lshr i32 %15, 3
  %17 = icmp eq i32 %4, 0
  br i1 %17, label %82, label %18

18:
  %19 = icmp eq i32 %0, 4
  %20 = icmp eq i32 %0, 19
  %21 = select i1 %9, i32 2, i32 0
  %22 = shl nuw nsw i32 %16, %21
  %23 = select i1 %19, i32 6, i32 7
  %24 = select i1 %20, i32 0, i32 %23
  br label %25

25:
  %26 = phi i32 [ 0, %18 ], [ %80, %79 ]
  %27 = phi ptr [ %5, %18 ], [ %81, %79 ]
  %28 = icmp eq i32 %16, 0
  br i1 %28, label %30, label %29

29:
  br label %34

30:
  %31 = icmp eq i32 %3, 0
  br i1 %31, label %79, label %32

32:
  %33 = add i32 %26, %2
  br label %40

34:
  %35 = phi ptr [ %27, %29 ], [ %38, %34 ]
  %36 = phi i32 [ %37, %34 ], [ 0, %29 ]
  store i8 0, ptr %35, align 1, !tbaa !12
  %37 = add nuw nsw i32 %36, 1
  %38 = getelementptr i8, ptr %35, i32 1
  %39 = icmp samesign ult i32 %37, %22
  br i1 %39, label %34, label %30, !llvm.loop !13

40:
  %41 = phi i32 [ 0, %32 ], [ %78, %77 ]
  %42 = lshr i32 %41, 3
  %43 = getelementptr i8, ptr %27, i32 %42
  %44 = add i32 %41, %1
  %45 = tail call i32 @gd_read(i32 noundef %44, i32 noundef %33) nobuiltin "no-builtins"
  %46 = icmp eq i32 %0, 16
  br i1 %46, label %49, label %47

47:
  %48 = icmp eq i32 %0, 13
  br i1 %48, label %49, label %67

49:
  %50 = and i32 %41, 7
  %51 = lshr exact i32 128, %50
  %52 = trunc nuw i32 %51 to i8
  br label %53

53:
  %54 = phi ptr [ %43, %49 ], [ %66, %62 ]
  %55 = phi i32 [ 0, %49 ], [ %63, %62 ]
  %56 = shl nuw nsw i32 1, %55
  %57 = and i32 %56, %45
  %58 = icmp eq i32 %57, 0
  br i1 %58, label %62, label %59

59:
  %60 = load i8, ptr %54, align 1, !tbaa !12
  %61 = or i8 %60, %52
  store i8 %61, ptr %54, align 1, !tbaa !12
  br label %62

62:
  %63 = add nuw nsw i32 %55, 1
  %64 = icmp samesign ult i32 %55, 3
  %65 = select i1 %9, i1 %64, i1 false
  %66 = getelementptr i8, ptr %54, i32 %16
  br i1 %65, label %53, label %77, !llvm.loop !15

67:
  %68 = shl i32 %41, %13
  %69 = and i32 %68, 7
  %70 = sub nsw i32 %24, %69
  %71 = shl i32 %45, %70
  %72 = lshr i32 %68, 3
  %73 = getelementptr inbounds nuw i8, ptr %27, i32 %72
  %74 = load i8, ptr %73, align 1, !tbaa !12
  %75 = trunc i32 %71 to i8
  %76 = or i8 %74, %75
  store i8 %76, ptr %73, align 1, !tbaa !12
  br label %77

77:
  %78 = add nuw i32 %41, 1
  %exitcond = icmp eq i32 %78, %3
  br i1 %exitcond, label %79, label %40, !llvm.loop !16

79:
  %80 = add nuw i32 %26, 1
  %81 = getelementptr inbounds nuw i8, ptr %27, i32 %22
  %exitcond1 = icmp eq i32 %80, %4
  br i1 %exitcond1, label %82, label %25, !llvm.loop !17

82:
  ret void
}

declare void @llvm.lifetime.start.p0(i64, ptr immarg) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare i32 @gd_read(i32 noundef, i32 noundef) "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000"

declare void @llvm.lifetime.end.p0(i64, ptr immarg) nocallback nofree nosync nounwind willreturn memory(argmem: write)

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
!8 = !{!5, !5, i64 0}
!9 = !{!10, !10, i64 0}
!10 = !{!"p1 omnipotent char", !11, i64 0}
!11 = !{!"any pointer", !6, i64 0}
!12 = !{!6, !6, i64 0}
!13 = distinct !{!13, !14}
!14 = !{!"llvm.loop.mustprogress"}
!15 = distinct !{!15, !14}
!16 = distinct !{!16, !14}
!17 = distinct !{!17, !14}
