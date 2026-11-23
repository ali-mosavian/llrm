target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"

@labels = internal global [4 x ptr] [ptr blockaddress(@run, %h0), ptr blockaddress(@run, %h1), ptr blockaddress(@run, %h2), ptr blockaddress(@run, %done)]

define void @run(ptr %pc, ptr %acc) {
entry:
  br label %dispatch

dispatch:
  %p = phi ptr [ %pc, %entry ], [ %next, %join ]
  %op = load i8, ptr %p
  %idx = zext i8 %op to i64
  %slot = getelementptr [4 x ptr], ptr @labels, i64 0, i64 %idx
  %target = load ptr, ptr %slot
  indirectbr ptr %target, [label %h0, label %h1, label %h2, label %done]

h0:
  %a0 = load i32, ptr %acc
  %b0 = add i32 %a0, 1
  store i32 %b0, ptr %acc
  %n0 = getelementptr i8, ptr %p, i64 1
  br label %join

h1:
  %a1 = load i32, ptr %acc
  %b1 = mul i32 %a1, 3
  store i32 %b1, ptr %acc
  %n1 = getelementptr i8, ptr %p, i64 2
  br label %join

h2:
  %a2 = load i32, ptr %acc
  %b2 = xor i32 %a2, 5
  store i32 %b2, ptr %acc
  %n2 = getelementptr i8, ptr %p, i64 3
  br label %join

join:
  %next = phi ptr [ %n0, %h0 ], [ %n1, %h1 ], [ %n2, %h2 ]
  br label %dispatch

done:
  ret void
}
