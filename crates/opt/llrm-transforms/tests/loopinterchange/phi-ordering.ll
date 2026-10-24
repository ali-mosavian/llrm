; ModuleID = '<stdin>'
source_filename = "<stdin>"
target datalayout = "e-m:e-p:32:32-i64:64-v128:64:128-a:0:32-n32-S64"

define void @test(i32 %T, ptr noalias nocapture %C, ptr noalias readonly nocapture %A, ptr noalias readonly nocapture %B) local_unnamed_addr {
entry:
  br label %for1.header

for1.header:                                      ; preds = %for1.inc19, %entry
  %i = phi i32 [ %inc20, %for1.inc19 ], [ 0, %entry ]
  br label %for2.header

for2.header:                                      ; preds = %for2.inc16, %for1.header
  %j = phi i32 [ 0, %for1.header ], [ %inc17, %for2.inc16 ]
  %gep.c = getelementptr inbounds [90 x i32], ptr %C, i32 %i, i32 %j
  %red.init = load i32, ptr %gep.c, align 4
  br label %for3

for3:                                             ; preds = %for3, %for2.header
  %red = phi i32 [ %red.init, %for2.header ], [ %add15, %for3 ]
  %k = phi i32 [ 1, %for2.header ], [ %inc, %for3 ]
  %arrayidx = getelementptr inbounds [90 x i16], ptr %A, i32 %i, i32 %k
  %0 = load i16, ptr %arrayidx, align 2
  %conv = sext i16 %0 to i32
  %add15 = add nsw i32 %conv, %red
  %inc = add nuw nsw i32 %k, 1
  %exitcond = icmp eq i32 %inc, 90
  br i1 %exitcond, label %for2.inc16, label %for3

for2.inc16:                                       ; preds = %for3
  %red.lcssa = phi i32 [ %red, %for3 ]
  store i32 %red.lcssa, ptr %gep.c, align 4
  %inc17 = add nuw nsw i32 %j, 1
  %exitcond47 = icmp eq i32 %inc17, 90
  br i1 %exitcond47, label %for1.inc19, label %for2.header

for1.inc19:                                       ; preds = %for2.inc16
  %inc20 = add nuw nsw i32 %i, 1
  %exitcond48 = icmp eq i32 %inc20, 90
  br i1 %exitcond48, label %for1.loopexit, label %for1.header

for1.loopexit:                                    ; preds = %for1.inc19
  br label %exit

exit:                                             ; preds = %for1.loopexit
  ret void
}
