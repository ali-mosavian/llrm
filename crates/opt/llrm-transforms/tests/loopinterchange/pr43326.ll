; ModuleID = '<stdin>'
source_filename = "<stdin>"

@a = global i32 0
@b = global i8 0
@c = global i32 0
@d = global i32 0
@e = global [1 x [1 x i32]] zeroinitializer

define void @pr43326() {
entry:
  %0 = load i32, ptr @a, align 4
  %tobool.not2 = icmp eq i32 %0, 0
  br i1 %tobool.not2, label %for.end14, label %outer.preheader

outer.preheader:                                  ; preds = %entry
  %d.promoted = load i32, ptr @d, align 4
  %a.promoted = load i32, ptr @a, align 4
  br label %outer.header

outer.header:                                     ; preds = %for.inc12, %outer.preheader
  %inc1312 = phi i32 [ %a.promoted, %outer.preheader ], [ %inc13, %for.inc12 ]
  %xor.lcssa.lcssa11 = phi i32 [ %d.promoted, %outer.preheader ], [ %xor.lcssa.lcssa, %for.inc12 ]
  br label %inner1.header

inner1.header:                                    ; preds = %for.inc10, %outer.header
  %xor.lcssa9 = phi i32 [ %xor.lcssa.lcssa11, %outer.header ], [ %xor.lcssa, %for.inc10 ]
  %j = phi i8 [ 0, %outer.header ], [ %j.next, %for.inc10 ]
  %idxprom8 = sext i8 %j to i64
  br label %inner2.header

inner2.header:                                    ; preds = %for.inc, %inner1.header
  %xor5 = phi i32 [ %xor.lcssa9, %inner1.header ], [ %xor, %for.inc ]
  %k = phi i32 [ 0, %inner1.header ], [ %k.next, %for.inc ]
  %idxprom = sext i32 %k to i64
  %arrayidx9 = getelementptr inbounds [1 x [1 x i32]], ptr @e, i64 0, i64 %idxprom, i64 %idxprom8
  %1 = load i32, ptr %arrayidx9, align 4
  %xor = xor i32 %xor5, %1
  br label %for.inc

for.inc:                                          ; preds = %inner2.header
  %k.next = add nsw i32 %k, 1
  %cmp5 = icmp slt i32 %k.next, 42
  br i1 %cmp5, label %inner2.header, label %for.end

for.end:                                          ; preds = %for.inc
  %xor.lcssa = phi i32 [ %xor, %for.inc ]
  %inc.lcssa = phi i32 [ %k.next, %for.inc ]
  br label %for.inc10

for.inc10:                                        ; preds = %for.end
  %j.next = add i8 %j, -1
  %cmp = icmp sgt i8 %j.next, -10
  br i1 %cmp, label %inner1.header, label %for.end11

for.end11:                                        ; preds = %for.inc10
  %xor.lcssa.lcssa = phi i32 [ %xor.lcssa, %for.inc10 ]
  %dec.lcssa = phi i8 [ %j.next, %for.inc10 ]
  %inc.lcssa.lcssa = phi i32 [ %inc.lcssa, %for.inc10 ]
  br label %for.inc12

for.inc12:                                        ; preds = %for.end11
  %inc13 = add nsw i32 %inc1312, 1
  %tobool.not = icmp slt i32 %inc13, 42
  br i1 %tobool.not, label %outer.header, label %for.cond.for.end14_crit_edge

for.cond.for.end14_crit_edge:                     ; preds = %for.inc12
  %inc13.lcssa = phi i32 [ %inc13, %for.inc12 ]
  %inc.lcssa.lcssa.lcssa = phi i32 [ %inc.lcssa.lcssa, %for.inc12 ]
  %xor.lcssa.lcssa.lcssa = phi i32 [ %xor.lcssa.lcssa, %for.inc12 ]
  %dec.lcssa.lcssa = phi i8 [ %dec.lcssa, %for.inc12 ]
  store i8 %dec.lcssa.lcssa, ptr @b, align 1
  store i32 %xor.lcssa.lcssa.lcssa, ptr @d, align 4
  store i32 %inc.lcssa.lcssa.lcssa, ptr @c, align 4
  store i32 %inc13.lcssa, ptr @a, align 4
  br label %for.end14

for.end14:                                        ; preds = %for.cond.for.end14_crit_edge, %entry
  ret void
}
