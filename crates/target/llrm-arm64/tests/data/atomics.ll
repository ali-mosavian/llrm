target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "arm64-apple-macosx27.2.0"

@x = global i32 0, align 4

define i32 @twice_seq_cst() {
  %a = load atomic i32, ptr @x seq_cst, align 4
  %b = load atomic i32, ptr @x seq_cst, align 4
  %c = add i32 %a, %b
  ret i32 %c
}

define void @unused_load() {
  %a = load atomic i32, ptr @x seq_cst, align 4
  ret void
}

define void @publish(i32 %v) {
  store i32 %v, ptr @x, align 4
  store atomic i32 1, ptr @x release, align 4
  ret void
}

define i32 @acquire() {
  %a = load atomic i32, ptr @x acquire, align 4
  ret i32 %a
}

define i32 @fetch_add(i32 %v) {
  %a = atomicrmw add ptr @x, i32 %v seq_cst, align 4
  ret i32 %a
}

define i8 @exchange(ptr %p, i8 %v) {
  %a = atomicrmw xchg ptr %p, i8 %v acquire, align 1
  ret i8 %a
}

define i32 @fetch_sub(ptr %p, i32 %v) {
  %a = atomicrmw sub ptr %p, i32 %v monotonic, align 4
  ret i32 %a
}

define i32 @fetch_and(ptr %p, i32 %v) {
  %a = atomicrmw and ptr %p, i32 %v release, align 4
  ret i32 %a
}

define i1 @compare_exchange(ptr %p, i32 %e, i32 %n) {
  %a = cmpxchg ptr %p, i32 %e, i32 %n seq_cst seq_cst, align 4
  %ok = extractvalue { i32, i1 } %a, 1
  ret i1 %ok
}

define void @fences() {
  fence seq_cst
  fence acquire
  ret void
}
