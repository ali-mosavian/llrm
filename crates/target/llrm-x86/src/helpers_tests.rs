//! The divides, run on a small interpreter of the instructions they are made of
//! and compared with the host's division.

use iced_x86::{Decoder, DecoderOptions, Instruction, Mnemonic, OpKind, Register};

use super::*;

/// 32-bit registers, the carry, zero and sign flags, and a stack.
struct Machine {
    registers: [u32; 8],
    carry: bool,
    zero: bool,
    sign: bool,
    stack: Vec<u32>,
}

fn index(register: impl Into<Register>) -> usize {
    match register.into().full_register32() {
        Register::EAX => 0,
        Register::ECX => 1,
        Register::EDX => 2,
        Register::EBX => 3,
        Register::ESP => 4,
        Register::EBP => 5,
        Register::ESI => 6,
        Register::EDI => 7,
        other => panic!("register {other:?}"),
    }
}

impl Machine {
    fn read(
        &self,
        instruction: &Instruction,
        at: u32,
    ) -> u32 {
        match instruction.op_kind(at) {
            OpKind::Register if instruction.op_register(at) == Register::CL => self.registers[1] & 0xFF,
            OpKind::Register => self.registers[index(instruction.op_register(at))],
            OpKind::Immediate8to32 => instruction.immediate8to32() as u32,
            OpKind::Immediate32 => instruction.immediate32(),
            OpKind::Immediate8 => u32::from(instruction.immediate8()),
            OpKind::NearBranch16 | OpKind::NearBranch32 => 0,
            other => panic!("operand {other:?}"),
        }
    }

    fn write(
        &mut self,
        instruction: &Instruction,
        value: u32,
    ) {
        self.registers[index(instruction.op0_register())] = value;
    }

    fn flags(
        &mut self,
        result: u32,
        carry: bool,
    ) {
        (self.carry, self.zero, self.sign) = (carry, result == 0, result >> 31 == 1);
    }

    fn subtract(
        &mut self,
        left: u32,
        right: u32,
        borrow: bool,
    ) -> u32 {
        let wide = u64::from(left).wrapping_sub(u64::from(right)).wrapping_sub(u64::from(borrow));
        let result = wide as u32;
        self.flags(result, wide >> 32 != 0);
        result
    }

    fn add(
        &mut self,
        left: u32,
        right: u32,
        carry: bool,
    ) -> u32 {
        let wide = u64::from(left) + u64::from(right) + u64::from(carry);
        let result = wide as u32;
        self.flags(result, wide >> 32 != 0);
        result
    }

    fn taken(
        &self,
        mnemonic: Mnemonic,
    ) -> bool {
        match mnemonic {
            Mnemonic::Jmp => true,
            Mnemonic::Jb => self.carry,
            Mnemonic::Jae => !self.carry,
            Mnemonic::Ja => !self.carry && !self.zero,
            Mnemonic::Jns => !self.sign,
            Mnemonic::Js => self.sign,
            Mnemonic::Jne => !self.zero,
            Mnemonic::Je => self.zero,
            other => panic!("jump {other:?}"),
        }
    }

    fn run(
        &mut self,
        code: &[u8],
        bits: u32,
    ) -> usize {
        let instructions: Vec<Instruction> =
            Decoder::with_ip(bits, code, 0, DecoderOptions::NONE).into_iter().collect();
        let mut at = 0;
        let mut steps = 0;
        while let Some(one) = instructions.iter().find(|one| one.ip() == at) {
            steps += 1;
            assert!(steps < 5000, "does not stop");
            at = one.next_ip();
            let left = if one.op_count() > 0 { self.read(one, 0) } else { 0 };
            let right = if one.op_count() > 1 { self.read(one, 1) } else { 0 };
            match one.mnemonic() {
                Mnemonic::Mov => self.write(one, right),
                Mnemonic::Xor => {
                    self.write(one, left ^ right);
                    self.flags(left ^ right, false)
                }
                Mnemonic::Or => {
                    self.write(one, left | right);
                    self.flags(left | right, false)
                }
                Mnemonic::Test => self.flags(left & right, false),
                Mnemonic::Cmp => {
                    self.subtract(left, right, false);
                }
                Mnemonic::Sub => {
                    let result = self.subtract(left, right, false);
                    self.write(one, result)
                }
                Mnemonic::Sbb => {
                    let result = self.subtract(left, right, self.carry);
                    self.write(one, result)
                }
                Mnemonic::Add => {
                    let result = self.add(left, right, false);
                    self.write(one, result)
                }
                Mnemonic::Adc => {
                    let result = self.add(left, right, self.carry);
                    self.write(one, result)
                }
                Mnemonic::Neg => {
                    let result = self.subtract(0, left, false);
                    self.write(one, result)
                }
                Mnemonic::Inc => self.write(one, left.wrapping_add(1)),
                Mnemonic::Dec => {
                    let result = left.wrapping_sub(1);
                    let carry = self.carry;
                    self.flags(result, carry);
                    self.write(one, result)
                }
                Mnemonic::Shr | Mnemonic::Shl => {
                    let count = right & 31;
                    let (result, carry) = match (one.mnemonic(), count) {
                        (_, 0) => (left, self.carry),
                        (Mnemonic::Shr, _) => (left >> count, left >> (count - 1) & 1 == 1),
                        (_, _) => (left << count, left >> (32 - count) & 1 == 1),
                    };
                    self.write(one, result);
                    self.flags(result, carry)
                }
                Mnemonic::Shld => {
                    let (count, source) = (self.read(one, 2) & 31, self.read(one, 1));
                    let result = if count == 0 { left } else { left << count | source >> (32 - count) };
                    self.write(one, result)
                }
                Mnemonic::Mul => {
                    let product = u64::from(self.registers[0]) * u64::from(left);
                    (self.registers[0], self.registers[2]) = (product as u32, (product >> 32) as u32);
                }
                Mnemonic::Imul => self.write(one, left.wrapping_mul(right)),
                Mnemonic::Rcr => {
                    assert_eq!(right, 1);
                    let result = left >> 1 | u32::from(self.carry) << 31;
                    self.write(one, result);
                    self.carry = left & 1 == 1;
                }
                Mnemonic::Stc => self.carry = true,
                Mnemonic::Clc => self.carry = false,
                Mnemonic::Xchg => {
                    self.registers[index(one.op0_register())] = right;
                    self.registers[index(one.op1_register())] = left;
                }
                Mnemonic::Push => self.stack.push(left),
                Mnemonic::Pop => {
                    let value = self.stack.pop().expect("a pushed value");
                    self.write(one, value)
                }
                Mnemonic::Div => {
                    let dividend = u64::from(self.registers[2]) << 32 | u64::from(self.registers[0]);
                    assert!(left != 0 && dividend / u64::from(left) <= u64::from(u32::MAX), "div faults");
                    self.registers[0] = (dividend / u64::from(left)) as u32;
                    self.registers[2] = (dividend % u64::from(left)) as u32;
                }
                mnemonic if one.is_jcc_short_or_near() || mnemonic == Mnemonic::Jmp => {
                    if self.taken(mnemonic) {
                        at = one.near_branch_target();
                    }
                }
                other => panic!("{other:?}"),
            }
        }
        steps
    }
}

fn divided(
    routine: &Divide,
    bits: u32,
    dividend: u64,
    divisor: u64,
) -> ((u64, u64), usize) {
    let mut machine = Machine { registers: [0; 8], carry: false, zero: false, sign: false, stack: Vec::new() };
    machine.registers[index(routine.dividend[0].iced())] = dividend as u32;
    machine.registers[index(routine.dividend[1].iced())] = (dividend >> 32) as u32;
    machine.registers[index(routine.divisor[0].iced())] = divisor as u32;
    machine.registers[index(routine.divisor[1].iced())] =
        if routine.wide_divisor { (divisor >> 32) as u32 } else { 0xDEAD };
    let (kept, before) = ([5, 6, 7].map(|at| 0x1000 + at as u32), machine.registers[4]);
    (machine.registers[5], machine.registers[6], machine.registers[7]) = (kept[0], kept[1], kept[2]);
    let steps = machine.run(&routine.code, bits);
    assert_eq!((machine.registers[5], machine.registers[6], machine.registers[7]), (kept[0], kept[1], kept[2]));
    assert!(machine.stack.is_empty() && machine.registers[4] == before, "the stack is balanced");
    let pair = |low: RegId, high: RegId| {
        u64::from(machine.registers[index(high)]) << 32 | u64::from(machine.registers[index(low)])
    };
    ((pair(routine.quotient[0], routine.quotient[1]), pair(routine.remainder[0], routine.remainder[1])), steps)
}

fn values() -> Vec<u64> {
    let edge = [
        0,
        1,
        2,
        3,
        7,
        10,
        0x7FFF_FFFF,
        0x8000_0000,
        0xFFFF_FFFF,
        0x1_0000_0000,
        0x1_0000_0001,
        0x1_FFFF_FFFF,
        0x7FFF_FFFF_FFFF_FFFF,
        0x8000_0000_0000_0000,
        0x8000_0000_0000_0001,
        u64::MAX,
        u64::MAX - 1,
        0x0000_0001_8000_0000,
        0xFFFF_FFFF_0000_0000,
        0x1234_5678_9ABC_DEF0,
    ];
    let mut state = 0x9E37_79B9_7F4A_7C15_u64;
    let mut random = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    let mut out = edge.to_vec();
    for _ in 0..600 {
        let shift = random() % 64;
        out.push(random() >> shift);
    }
    out
}

/// Every divide of the four, in each code width, equals the host's, and keeps
/// what it promises to keep.
#[test]
fn the_divides_equal_the_hosts() {
    for bits in [16, 32] {
        for (signed, wide_divisor) in [(false, true), (true, true), (false, false), (true, false)] {
            let routine = divide(signed, wide_divisor, bits).expect("assembles");
            for &dividend in &values() {
                for &divisor in &values() {
                    if divisor == 0 || (!wide_divisor && (divisor >> 32 != 0)) {
                        continue;
                    }
                    let ((quotient, remainder), _) = divided(&routine, bits, dividend, divisor);
                    let want = if signed {
                        let (a, b) = (dividend as i64, divisor as i64);
                        (a.wrapping_div(b) as u64, a.wrapping_rem(b) as u64)
                    } else {
                        (dividend / divisor, dividend % divisor)
                    };
                    assert_eq!(
                        (quotient, remainder),
                        want,
                        "{} {dividend:#x} / {divisor:#x} at {} bits",
                        routine.name,
                        bits
                    );
                }
            }
        }
    }
}

/// The signed divide by a dword divisor fixed the sign of both results for a
/// caller that reads one (SD232): data_int64 pasted 87 bytes of routine for a
/// quotient. A routine for the result read is shorter and gives that result as
/// the host does.
#[test]
fn a_divide_read_for_one_result_is_shorter_and_that_result_is_right() {
    for bits in [16, 32] {
        let both = divide(true, false, bits).expect("assembles");
        for (used, name) in [(Used::Quotient, "div64_s32q"), (Used::Remainder, "div64_s32r")] {
            let routine = divide_used(true, false, bits, used).expect("assembles");
            assert_eq!(routine.name, name);
            assert!(
                routine.code.len() + 6 <= both.code.len(),
                "{name}: {} of {} bytes",
                routine.code.len(),
                both.code.len()
            );
            for &dividend in &values() {
                for &divisor in values().iter().filter(|&&one| one != 0 && one >> 32 == 0) {
                    let ((quotient, remainder), _) = divided(&routine, bits, dividend, divisor);
                    let (a, b) = (dividend as i64, divisor as i64);
                    let (got, want) = match used {
                        Used::Quotient => (quotient, a.wrapping_div(b) as u64),
                        _ => (remainder, a.wrapping_rem(b) as u64),
                    };
                    assert_eq!(got, want, "{name} {dividend:#x} / {divisor:#x} at {bits} bits");
                }
            }
        }
    }
}

/// A divide by a divisor of two dwords estimates the quotient with one `div`
/// and corrects it: 144 instructions at the worst, where a round per bit of
/// the quotient is some 600.
#[test]
fn a_wide_divide_is_bounded_not_a_round_per_bit() {
    for (signed, dividend, divisor) in [
        (false, u64::MAX, 0x1_0000_0001),
        (false, 0x1234_5678_9ABC_DEF0, 0x1_0000_0001),
        (true, u64::MAX >> 1, 0x1_0001_0000),
    ] {
        let routine = divide(signed, true, 16).expect("assembles");
        let (_, steps) = divided(&routine, 16, dividend, divisor);
        assert!(steps < 160, "{} took {steps} instructions", routine.name);
    }
}

/// The huge string routines assemble for real mode, end where they began (`ds`
/// and the direction flag are kept), and carry their loops' labels inside their
/// own bytes.
#[test]
fn test_the_huge_string_routines_assemble() {
    for routine in [
        huge_fill(1).unwrap(),
        huge_fill(2).unwrap(),
        huge_fill(4).unwrap(),
        huge_copy(false).unwrap(),
        huge_copy(true).unwrap(),
    ] {
        assert!(routine.code.len() > 40 && routine.code.len() < 200, "{}: {} bytes", routine.name, routine.code.len());
        assert!(routine.reads.contains(&RegId::ECX), "{}", routine.name);
        assert!(routine.clobbers.contains(&RegId::ES), "{}", routine.name);
    }
}

/// Every call site of a huge-pointer or 64-bit divide routine assembled its
/// source again ("huge_copy/fill helper assembly" 48 M and "M32 divide helper
/// assembly" 60 M of the dag path's compile): a block is assembled once.
#[test]
fn a_helper_asked_for_twice_is_assembled_once() {
    let before = crate::asm::ASSEMBLED.with(|count| count.get());
    let first = crate::asm::assembled(
        &["    mov eax, 0x5a5a1234", "    ret"],
        crate::asm::Mode { bits: 32, segmented: false, address_bytes: 4, wide: true },
    );
    let again = crate::asm::assembled(
        &["    mov eax, 0x5a5a1234", "    ret"],
        crate::asm::Mode { bits: 32, segmented: false, address_bytes: 4, wide: true },
    );
    assert_eq!(first, again);
    assert_eq!(crate::asm::ASSEMBLED.with(|count| count.get()) - before, 1);
}

/// The selector assembles a routine only for a call whose name starts with
/// the routine kind's prefix: a routine named otherwise would be called and
/// never laid down.
#[test]
fn every_routine_name_starts_with_its_kinds_prefix() {
    use llrm_target::routines::{DIVIDE_PREFIX, HUGE_PREFIX};
    for signed in [true, false] {
        for wide in [true, false] {
            for bits in [16, 32] {
                let name = divide(signed, wide, bits).expect("a divide").name;
                assert!(name.starts_with(DIVIDE_PREFIX), "{name}");
            }
        }
    }
    for cell in [1, 2, 4] {
        let name = huge_fill(cell).expect("a fill").name;
        assert!(name.starts_with(HUGE_PREFIX), "{name}");
    }
    for down in [false, true] {
        let name = huge_copy(down).expect("a copy").name;
        assert!(name.starts_with(HUGE_PREFIX), "{name}");
    }
}
