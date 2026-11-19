define fp128 @add(fp128 %a, fp128 %b) { %r = fadd fp128 %a, %b ret fp128 %r }
define fp128 @sub(fp128 %a, fp128 %b) { %r = fsub fp128 %a, %b ret fp128 %r }
define fp128 @mul(fp128 %a, fp128 %b) { %r = fmul fp128 %a, %b ret fp128 %r }
define fp128 @fromi(i32 %a) { %r = sitofp i32 %a to fp128 ret fp128 %r }
define i64 @toi(fp128 %a) { %r = fptosi fp128 %a to i64 ret i64 %r }
define i64 @tou(fp128 %a) { %r = fptoui fp128 %a to i64 ret i64 %r }
define void @st(ptr %p, fp128 %a) { store fp128 %a, ptr %p, align 16 ret void }
define fp128 @ld(ptr %p) { %r = load fp128, ptr %p, align 16 ret fp128 %r }
