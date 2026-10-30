; ModuleID = '<stdin>'
source_filename = "<stdin>"

define i16 @test_it() {
entry:
  br label %for.cond

for.cond.loopexit:                                ; preds = %do.cond
  br label %for.cond

for.cond:                                         ; preds = %for.cond.loopexit, %entry
  br label %do.body2

do.body2:                                         ; preds = %do.cond, %for.cond
  br label %while.cond3

while.cond3:                                      ; preds = %while.cond3, %do.body2
  br i1 true, label %do.cond, label %while.cond3

do.cond:                                          ; preds = %while.cond3
  br i1 true, label %for.cond.loopexit, label %do.body2
}
