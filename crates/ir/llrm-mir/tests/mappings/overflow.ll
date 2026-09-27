; An error check BC's code makes: overflow by llvm.sadd.with.overflow, and
; @llrm.qb.error, noreturn but able to unwind to a handler. LLVM 20's
; interpreter has no overflow intrinsics.
; expect: none
target datalayout = "e-p:16:16-p1:32:16:16:16-i32:16-i64:16"

declare { i16, i1 } @llvm.sadd.with.overflow.i16(i16, i16)
declare void @llrm.qb.error(i16) noreturn

define i16 @checked_add(i16 %a, i16 %b) {
entry:
  %pair = call { i16, i1 } @llvm.sadd.with.overflow.i16(i16 %a, i16 %b)
  %over = extractvalue { i16, i1 } %pair, 1
  br i1 %over, label %fail, label %ok
fail:
  call void @llrm.qb.error(i16 6)
  unreachable
ok:
  %sum = extractvalue { i16, i1 } %pair, 0
  ret i16 %sum
}
