; ModuleID = '<stdin>'
source_filename = "<stdin>"
target datalayout = "e-m:e-i64:64-f80:128-n8:16:32:64-S128"

@D = common global [100 x [100 x [100 x i32]]] zeroinitializer

define void @interchange_i_and_j(i32 %t) {
entry:
  br label %outer.header

outer.header:                                     ; preds = %for.inc15, %entry
  %i = phi i64 [ 0, %entry ], [ %inc16, %for.inc15 ]
  br label %inner1.header

inner1.header:                                    ; preds = %for.inc12, %outer.header
  %j = phi i64 [ 0, %outer.header ], [ %inc13, %for.inc12 ]
  br label %inner2.body

inner2.body:                                      ; preds = %inner2.body, %inner1.header
  %k = phi i64 [ 0, %inner1.header ], [ %inc, %inner2.body ]
  %arrayidx8 = getelementptr inbounds [100 x [100 x i32]], ptr @D, i64 %i, i64 %k, i64 %j
  %0 = load i32, ptr %arrayidx8, align 4
  %add = add nsw i32 %0, %t
  store i32 %add, ptr %arrayidx8, align 4
  %inc = add nuw nsw i64 %k, 1
  %exitcond = icmp eq i64 %inc, 100
  br i1 %exitcond, label %for.inc12, label %inner2.body

for.inc12:                                        ; preds = %inner2.body
  %inc13 = add nuw nsw i64 %j, 1
  %exitcond29 = icmp eq i64 %inc13, 100
  br i1 %exitcond29, label %for.inc15, label %inner1.header

for.inc15:                                        ; preds = %for.inc12
  %inc16 = add nuw nsw i64 %i, 1
  %exitcond30 = icmp eq i64 %inc16, 1
  br i1 %exitcond30, label %for.end17, label %outer.header

for.end17:                                        ; preds = %for.inc15
  ret void
}
