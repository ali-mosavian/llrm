; ModuleID = '<stdin>'
source_filename = "<stdin>"
target datalayout = "e-m:e-p:32:32-i64:64-v128:64:128-a:0:32-n32-S64"

define void @test1() {
bb:
  br label %bb5.preheader

bb5.preheader:                                    ; preds = %bb38, %bb
  %i10 = phi i16 [ 0, %bb ], [ %i42, %bb38 ]
  br label %bb10.preheader

bb10.preheader:                                   ; preds = %bb10.preheader, %bb5.preheader
  br i1 true, label %bb38, label %bb10.preheader

bb38:                                             ; preds = %bb10.preheader
  %i4017 = zext i16 %i10 to i64
  %i13.i = add i64 %i4017, 1
  %i14.i = select i1 true, i64 %i13.i, i64 %i4017
  %i42 = trunc i64 %i14.i to i16
  %i3.not = icmp eq i16 %i10, -27
  br i1 %i3.not, label %bb1.bb43_crit_edge, label %bb5.preheader

bb1.bb43_crit_edge:                               ; preds = %bb38
  ret void
}
