//! -Os left code larger than -O2 where the pin's optsize threshold of 0 refuses
//! what folds the rest: a fill-and-sum of five ints (64 B against 4: `moveq
//! #35`) and a nest whose inner loop sets `b` to a constant (54 B against 40,
//! the peel `computePeelCount` never takes at optsize). The -Os trial
//! (UNROLL-OS-TRIAL, PEEL-OS-TRIAL) compiles them and keeps what is smaller.

use llrm_m68k::driver::{Level, cpu_of, object};

const NEST: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
define i8 @g(i8 %a, i8 %b, i8 %m) ATTR {
entry:
  br label %oh
oh:
  %i = phi i8 [0, %entry], [%i.next, %ol]
  %a1 = phi i8 [%a, %entry], [%b2, %ol]
  %b1 = phi i8 [%b, %entry], [%b2, %ol]
  %mz = zext i8 %m to i32
  %iz = zext i8 %i to i32
  %c = icmp slt i32 %iz, %mz
  br i1 %c, label %ob, label %exit
ob:
  %m1 = add i8 %m, 1
  %m1z = zext i8 %m1 to i32
  br label %ih
ih:
  %j = phi i8 [0, %ob], [%j.next, %ibody]
  %b2 = phi i8 [%b1, %ob], [7, %ibody]
  %jz = zext i8 %j to i32
  %c2 = icmp slt i32 %jz, %m1z
  br i1 %c2, label %ibody, label %ol
ibody:
  %j.next = add i8 %j, 1
  br label %ih
ol:
  %i.next = add i8 %i, 1
  br label %oh
exit:
  %r = add i8 %a1, %b1
  ret i8 %r
}";

const FILL_AND_SUM: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
define i32 @g() ATTR {
entry:
  %v = alloca [5 x i32], align 4
  br label %fill
fill:
  %i = phi i32 [0, %entry], [%i1, %fill]
  %m = mul i32 %i, 3
  %x = add i32 %m, 1
  %p = getelementptr inbounds [5 x i32], ptr %v, i32 0, i32 %i
  store i32 %x, ptr %p
  %i1 = add i32 %i, 1
  %c = icmp ult i32 %i1, 5
  br i1 %c, label %fill, label %sum
sum:
  %j = phi i32 [0, %fill], [%j1, %sum]
  %s = phi i32 [0, %fill], [%s1, %sum]
  %q = getelementptr inbounds [5 x i32], ptr %v, i32 0, i32 %j
  %l = load i32, ptr %q
  %s1 = add i32 %s, %l
  %j1 = add i32 %j, 1
  %d = icmp ult i32 %j1, 5
  br i1 %d, label %sum, label %done
done:
  ret i32 %s1
}";

fn bytes(
    source: &str,
    cpu: &str,
    level: Level,
) -> usize {
    let attr = if level == Level::Os { "optsize minsize" } else { "" };
    object(&source.replace("ATTR", attr), cpu_of(cpu).unwrap(), level, "t.ll").expect("compiles").len()
}

#[test]
fn os_is_no_larger_than_o2_on_a_nest_whose_peel_folds() {
    for cpu in ["68000", "68020"] {
        assert!(bytes(NEST, cpu, Level::Os) <= bytes(NEST, cpu, Level::O2), "{cpu}: -Os larger than -O2");
    }
}

#[test]
fn os_is_no_larger_than_o2_on_a_fill_and_sum_of_five() {
    for cpu in ["68000", "68020"] {
        assert!(
            bytes(FILL_AND_SUM, cpu, Level::Os) <= bytes(FILL_AND_SUM, cpu, Level::O2),
            "{cpu}: -Os larger than -O2"
        );
    }
}
