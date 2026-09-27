; ModuleID = '/home/alim/work/personal/llrm-zed/bench/parity/qbsp.c'
source_filename = "/home/alim/work/personal/llrm-zed/bench/parity/qbsp.c"
target datalayout = "e-m:e-p:16:16-i32:16-i64:16-f32:16-f64:16-a:8-n8:16-S16"

%struct.Node = type { i16, i16, i16 }
%struct.Plane = type { %struct.Vec3, float }
%struct.Vec3 = type { float, float, float }

; Function Attrs: mustprogress nofree norecurse nosync nounwind willreturn memory(argmem: read)
define float @r_plane_dist(ptr nocapture noundef readonly %p, ptr nocapture noundef readonly %pl) local_unnamed_addr #0 {
entry:
  %0 = load float, ptr %p, align 2, !tbaa !2
  %1 = load float, ptr %pl, align 2, !tbaa !7
  %y = getelementptr inbounds nuw i8, ptr %p, i16 4
  %2 = load float, ptr %y, align 2, !tbaa !9
  %y3 = getelementptr inbounds nuw i8, ptr %pl, i16 4
  %3 = load float, ptr %y3, align 2, !tbaa !10
  %mul4 = fmul float %2, %3
  %4 = tail call float @llvm.fmuladd.f32(float %0, float %1, float %mul4)
  %z = getelementptr inbounds nuw i8, ptr %p, i16 8
  %5 = load float, ptr %z, align 2, !tbaa !11
  %z6 = getelementptr inbounds nuw i8, ptr %pl, i16 8
  %6 = load float, ptr %z6, align 2, !tbaa !12
  %7 = tail call float @llvm.fmuladd.f32(float %5, float %6, float %4)
  %dist = getelementptr inbounds nuw i8, ptr %pl, i16 12
  %8 = load float, ptr %dist, align 2, !tbaa !13
  %sub = fsub float %7, %8
  ret float %sub
}

; Function Attrs: mustprogress nocallback nofree nosync nounwind speculatable willreturn memory(none)
declare float @llvm.fmuladd.f32(float, float, float) #1

; Function Attrs: nofree norecurse nosync nounwind memory(argmem: read)
define signext i16 @r_point_leaf(ptr nocapture noundef readonly %p, ptr nocapture noundef readonly %nodes, ptr nocapture noundef readonly %planes) local_unnamed_addr #2 {
entry:
  %0 = load float, ptr %p, align 2, !tbaa !2
  %y.i = getelementptr inbounds nuw i8, ptr %p, i16 4
  %1 = load float, ptr %y.i, align 2, !tbaa !9
  %z.i = getelementptr inbounds nuw i8, ptr %p, i16 8
  %2 = load float, ptr %z.i, align 2, !tbaa !11
  br label %while.body

while.body:                                       ; preds = %entry, %while.body
  %nodenr.06 = phi i16 [ 0, %entry ], [ %nodenr.1, %while.body ]
  %arrayidx = getelementptr inbounds nuw %struct.Node, ptr %nodes, i16 %nodenr.06
  %3 = load i16, ptr %arrayidx, align 2, !tbaa !14
  %arrayidx1 = getelementptr inbounds %struct.Plane, ptr %planes, i16 %3
  %4 = load float, ptr %arrayidx1, align 2, !tbaa !7
  %y3.i = getelementptr inbounds nuw i8, ptr %arrayidx1, i16 4
  %5 = load float, ptr %y3.i, align 2, !tbaa !10
  %mul4.i = fmul float %1, %5
  %6 = tail call float @llvm.fmuladd.f32(float %0, float %4, float %mul4.i)
  %z6.i = getelementptr inbounds nuw i8, ptr %arrayidx1, i16 8
  %7 = load float, ptr %z6.i, align 2, !tbaa !12
  %8 = tail call float @llvm.fmuladd.f32(float %2, float %7, float %6)
  %dist.i = getelementptr inbounds nuw i8, ptr %arrayidx1, i16 12
  %9 = load float, ptr %dist.i, align 2, !tbaa !13
  %sub.i = fsub float %8, %9
  %cmp = fcmp ult float %sub.i, 0.000000e+00
  %nodenr.1.in.v = select i1 %cmp, i16 4, i16 2
  %nodenr.1.in = getelementptr inbounds nuw i8, ptr %arrayidx, i16 %nodenr.1.in.v
  %nodenr.1 = load i16, ptr %nodenr.1.in, align 2, !tbaa !17
  %tobool.not = icmp sgt i16 %nodenr.1, -1
  br i1 %tobool.not, label %while.body, label %while.end, !llvm.loop !18

while.end:                                        ; preds = %while.body
  %not = xor i16 %nodenr.1, -1
  ret i16 %not
}

; Function Attrs: mustprogress nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(i64 immarg, ptr nocapture) #3

; Function Attrs: mustprogress nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(i64 immarg, ptr nocapture) #3

; Function Attrs: nofree norecurse nosync nounwind memory(none)
define range(i32 0, 3637138) i32 @quake_bsp_demo() local_unnamed_addr #4 {
entry:
  %nodes = alloca [2 x %struct.Node], align 2
  %planes = alloca [2 x %struct.Plane], align 2
  call void @llvm.lifetime.start.p0(i64 12, ptr nonnull %nodes) #6
  call void @llvm.lifetime.start.p0(i64 32, ptr nonnull %planes) #6
  store float 1.000000e+00, ptr %planes, align 2, !tbaa !7
  %y = getelementptr inbounds nuw i8, ptr %planes, i16 4
  %y11 = getelementptr inbounds nuw i8, ptr %planes, i16 20
  call void @llvm.memset.p0.i64(ptr noundef nonnull align 2 dereferenceable(16) %y, i8 0, i64 16, i1 false)
  store float 1.000000e+00, ptr %y11, align 2, !tbaa !10
  %z14 = getelementptr inbounds nuw i8, ptr %planes, i16 24
  store float 0.000000e+00, ptr %z14, align 2, !tbaa !12
  %dist16 = getelementptr inbounds nuw i8, ptr %planes, i16 28
  store float 0.000000e+00, ptr %dist16, align 2, !tbaa !13
  store i16 0, ptr %nodes, align 2, !tbaa !14
  %child0 = getelementptr inbounds nuw i8, ptr %nodes, i16 2
  store i16 1, ptr %child0, align 2, !tbaa !21
  %child1 = getelementptr inbounds nuw i8, ptr %nodes, i16 4
  store i16 -1, ptr %child1, align 2, !tbaa !22
  %arrayidx20 = getelementptr inbounds nuw i8, ptr %nodes, i16 6
  store i16 1, ptr %arrayidx20, align 2, !tbaa !14
  %child023 = getelementptr inbounds nuw i8, ptr %nodes, i16 8
  store i16 -2, ptr %child023, align 2, !tbaa !21
  %child125 = getelementptr inbounds nuw i8, ptr %nodes, i16 10
  store i16 -3, ptr %child125, align 2, !tbaa !22
  br label %while.body.i

while.body.i:                                     ; preds = %while.body.i, %entry
  %nodenr.06.i = phi i16 [ 0, %entry ], [ %nodenr.1.i, %while.body.i ]
  %arrayidx.i = getelementptr inbounds nuw %struct.Node, ptr %nodes, i16 %nodenr.06.i
  %0 = load i16, ptr %arrayidx.i, align 2, !tbaa !14
  %arrayidx1.i = getelementptr inbounds %struct.Plane, ptr %planes, i16 %0
  %1 = load float, ptr %arrayidx1.i, align 2, !tbaa !7
  %y3.i.i = getelementptr inbounds nuw i8, ptr %arrayidx1.i, i16 4
  %2 = load float, ptr %y3.i.i, align 2, !tbaa !10
  %mul4.i.i = fmul float %2, 3.000000e+00
  %3 = tail call float @llvm.fmuladd.f32(float %1, float 2.000000e+00, float %mul4.i.i)
  %z6.i.i = getelementptr inbounds nuw i8, ptr %arrayidx1.i, i16 8
  %4 = load float, ptr %z6.i.i, align 2, !tbaa !12
  %5 = tail call float @llvm.fmuladd.f32(float %4, float 0.000000e+00, float %3)
  %dist.i.i = getelementptr inbounds nuw i8, ptr %arrayidx1.i, i16 12
  %6 = load float, ptr %dist.i.i, align 2, !tbaa !13
  %sub.i.i = fsub float %5, %6
  %cmp.i = fcmp ult float %sub.i.i, 0.000000e+00
  %nodenr.1.in.v.i.sroa.sel.v.sroa.sel.v.sroa.sel.v = select i1 %cmp.i, i16 4, i16 2
  %nodenr.1.in.v.i.sroa.sel.v.sroa.sel.v.sroa.sel = getelementptr inbounds nuw i8, ptr %arrayidx.i, i16 %nodenr.1.in.v.i.sroa.sel.v.sroa.sel.v.sroa.sel.v
  %nodenr.1.i = load i16, ptr %nodenr.1.in.v.i.sroa.sel.v.sroa.sel.v.sroa.sel, align 2, !tbaa !17
  %tobool.not.i = icmp sgt i16 %nodenr.1.i, -1
  br i1 %tobool.not.i, label %while.body.i, label %while.body.i44, !llvm.loop !18

while.body.i44:                                   ; preds = %while.body.i, %while.body.i44
  %nodenr.06.i45 = phi i16 [ %nodenr.1.i56, %while.body.i44 ], [ 0, %while.body.i ]
  %arrayidx.i46 = getelementptr inbounds nuw %struct.Node, ptr %nodes, i16 %nodenr.06.i45
  %7 = load i16, ptr %arrayidx.i46, align 2, !tbaa !14
  %arrayidx1.i47 = getelementptr inbounds %struct.Plane, ptr %planes, i16 %7
  %8 = load float, ptr %arrayidx1.i47, align 2, !tbaa !7
  %y3.i.i48 = getelementptr inbounds nuw i8, ptr %arrayidx1.i47, i16 4
  %9 = load float, ptr %y3.i.i48, align 2, !tbaa !10
  %mul4.i.i49 = fmul float %9, -3.000000e+00
  %10 = tail call float @llvm.fmuladd.f32(float %8, float 2.000000e+00, float %mul4.i.i49)
  %z6.i.i50 = getelementptr inbounds nuw i8, ptr %arrayidx1.i47, i16 8
  %11 = load float, ptr %z6.i.i50, align 2, !tbaa !12
  %12 = tail call float @llvm.fmuladd.f32(float %11, float 0.000000e+00, float %10)
  %dist.i.i51 = getelementptr inbounds nuw i8, ptr %arrayidx1.i47, i16 12
  %13 = load float, ptr %dist.i.i51, align 2, !tbaa !13
  %sub.i.i52 = fsub float %12, %13
  %cmp.i53 = fcmp ult float %sub.i.i52, 0.000000e+00
  %nodenr.1.in.v.i54.sroa.sel.v.sroa.sel.v.sroa.sel.v = select i1 %cmp.i53, i16 4, i16 2
  %nodenr.1.in.v.i54.sroa.sel.v.sroa.sel.v.sroa.sel = getelementptr inbounds nuw i8, ptr %arrayidx.i46, i16 %nodenr.1.in.v.i54.sroa.sel.v.sroa.sel.v.sroa.sel.v
  %nodenr.1.i56 = load i16, ptr %nodenr.1.in.v.i54.sroa.sel.v.sroa.sel.v.sroa.sel, align 2, !tbaa !17
  %tobool.not.i57 = icmp sgt i16 %nodenr.1.i56, -1
  br i1 %tobool.not.i57, label %while.body.i44, label %while.body.i62, !llvm.loop !18

while.body.i62:                                   ; preds = %while.body.i44, %while.body.i62
  %nodenr.06.i63 = phi i16 [ %nodenr.1.i74, %while.body.i62 ], [ 0, %while.body.i44 ]
  %arrayidx.i64 = getelementptr inbounds nuw %struct.Node, ptr %nodes, i16 %nodenr.06.i63
  %14 = load i16, ptr %arrayidx.i64, align 2, !tbaa !14
  %arrayidx1.i65 = getelementptr inbounds %struct.Plane, ptr %planes, i16 %14
  %15 = load float, ptr %arrayidx1.i65, align 2, !tbaa !7
  %y3.i.i66 = getelementptr inbounds nuw i8, ptr %arrayidx1.i65, i16 4
  %16 = load float, ptr %y3.i.i66, align 2, !tbaa !10
  %mul4.i.i67 = fmul float %16, -3.000000e+00
  %17 = tail call float @llvm.fmuladd.f32(float %15, float -2.000000e+00, float %mul4.i.i67)
  %z6.i.i68 = getelementptr inbounds nuw i8, ptr %arrayidx1.i65, i16 8
  %18 = load float, ptr %z6.i.i68, align 2, !tbaa !12
  %19 = tail call float @llvm.fmuladd.f32(float %18, float 0.000000e+00, float %17)
  %dist.i.i69 = getelementptr inbounds nuw i8, ptr %arrayidx1.i65, i16 12
  %20 = load float, ptr %dist.i.i69, align 2, !tbaa !13
  %sub.i.i70 = fsub float %19, %20
  %cmp.i71 = fcmp ult float %sub.i.i70, 0.000000e+00
  %nodenr.1.in.v.i72.sroa.sel.v.sroa.sel.v.sroa.sel.v = select i1 %cmp.i71, i16 4, i16 2
  %nodenr.1.in.v.i72.sroa.sel.v.sroa.sel.v.sroa.sel = getelementptr inbounds nuw i8, ptr %arrayidx.i64, i16 %nodenr.1.in.v.i72.sroa.sel.v.sroa.sel.v.sroa.sel.v
  %nodenr.1.i74 = load i16, ptr %nodenr.1.in.v.i72.sroa.sel.v.sroa.sel.v.sroa.sel, align 2, !tbaa !17
  %tobool.not.i75 = icmp sgt i16 %nodenr.1.i74, -1
  br i1 %tobool.not.i75, label %while.body.i62, label %r_point_leaf.exit77, !llvm.loop !18

r_point_leaf.exit77:                              ; preds = %while.body.i62
  %not.i58 = xor i16 %nodenr.1.i56, -1
  %not.i = xor i16 %nodenr.1.i, -1
  %not.i76 = xor i16 %nodenr.1.i74, -1
  %conv = zext nneg i16 %not.i to i32
  %mul = mul nuw nsw i32 %conv, 100
  %conv38 = zext nneg i16 %not.i58 to i32
  %mul39 = mul nuw nsw i32 %conv38, 10
  %add = add nuw nsw i32 %mul39, %mul
  %conv40 = zext nneg i16 %not.i76 to i32
  %add41 = add nuw nsw i32 %add, %conv40
  call void @llvm.lifetime.end.p0(i64 32, ptr nonnull %planes) #6
  call void @llvm.lifetime.end.p0(i64 12, ptr nonnull %nodes) #6
  ret i32 %add41
}

; Function Attrs: nocallback nofree nounwind willreturn memory(argmem: write)
declare void @llvm.memset.p0.i64(ptr nocapture writeonly, i8, i64, i1 immarg) #5

attributes #0 = { mustprogress nofree norecurse nosync nounwind willreturn memory(argmem: read) "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #1 = { mustprogress nocallback nofree nosync nounwind speculatable willreturn memory(none) }
attributes #2 = { nofree norecurse nosync nounwind memory(argmem: read) "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #3 = { mustprogress nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #4 = { nofree norecurse nosync nounwind memory(none) "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #5 = { nocallback nofree nounwind willreturn memory(argmem: write) }
attributes #6 = { nounwind }

!llvm.module.flags = !{!0}
!llvm.ident = !{!1}

!0 = !{i32 1, !"wchar_size", i32 2}
!1 = !{!"Ubuntu clang version 20.1.8 (0ubuntu4)"}
!2 = !{!3, !4, i64 0}
!3 = !{!"", !4, i64 0, !4, i64 4, !4, i64 8}
!4 = !{!"float", !5, i64 0}
!5 = !{!"omnipotent char", !6, i64 0}
!6 = !{!"Simple C/C++ TBAA"}
!7 = !{!8, !4, i64 0}
!8 = !{!"", !3, i64 0, !4, i64 12}
!9 = !{!3, !4, i64 4}
!10 = !{!8, !4, i64 4}
!11 = !{!3, !4, i64 8}
!12 = !{!8, !4, i64 8}
!13 = !{!8, !4, i64 12}
!14 = !{!15, !16, i64 0}
!15 = !{!"", !16, i64 0, !16, i64 2, !16, i64 4}
!16 = !{!"short", !5, i64 0}
!17 = !{!16, !16, i64 0}
!18 = distinct !{!18, !19, !20}
!19 = !{!"llvm.loop.mustprogress"}
!20 = !{!"llvm.loop.unroll.disable"}
!21 = !{!15, !16, i64 2}
!22 = !{!15, !16, i64 4}
