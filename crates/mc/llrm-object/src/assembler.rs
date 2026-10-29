//! What the assembler asks of a target (LLVM's `MCAsmBackend`).

use std::collections::HashMap;

use crate::{FixupKind, FixupKindInfo, Unsupported};

pub trait AsmBackend {
    /// What `kind` says of the field it patches.
    fn kind_info(
        &self,
        kind: FixupKind,
    ) -> FixupKindInfo;

    /// Appends `count` bytes that do nothing (`writeNopData`); false where the
    /// target has none of that length.
    fn write_nop_data(
        &self,
        out: &mut Vec<u8>,
        count: usize,
    ) -> bool;

    /// Mach-O's compact unwind encoding for a function whose prologue the
    /// call frame instructions `cfi` describe
    /// (`MCAsmBackend::generateCompactUnwindEncoding`); 0 where the target has
    /// none, which leaves the function to its DWARF frame.
    fn generate_compact_unwind_encoding(
        &self,
        _cfi: &[Cfi],
    ) -> u32 {
        0
    }
}

/// A call frame instruction (`MCCFIInstruction`), the operations a prologue
/// states: the register and offset the canonical frame address is, and where a
/// register is saved.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Cfi {
    DefCfa {
        register: u16,
        offset: i64,
    },
    DefCfaRegister {
        register: u16,
    },
    DefCfaOffset {
        offset: i64,
    },
    Offset {
        register: u16,
        offset: i64,
    },
    /// The register is back where the caller left it (`.cfi_restore`).
    Restore {
        register: u16,
    },
}

/// A piece of a section as the assembler lays it out (`MCFragment`): a label,
/// fixed bytes, padding that depends on where it lands, or a jump that grows.
pub trait Fragment {
    /// What the layout knows besides the fragments (the code model).
    type Context;

    /// The name this fragment defines at its offset, if it does.
    fn label(&self) -> Option<&str>;

    /// Its bytes where it lies at offset `at`.
    fn size(
        &self,
        at: i64,
        context: &Self::Context,
    ) -> usize;

    /// Lets it grow once the offsets of the fragments are known
    /// (`MCAssembler::relaxFragment`); whether it changed.
    fn relax(
        &mut self,
        at: i64,
        labels: &HashMap<String, i64>,
        context: &Self::Context,
    ) -> Result<bool, Unsupported>;
}

/// The offset of every label, in the order the fragments define them.
pub type Labels = Vec<(String, i64)>;

/// `MCAssembler::layout`: sizes every fragment, relaxes those that can grow,
/// and again until none does. Growth only moves targets further away, so it
/// ends, at the smallest layout.
pub fn layout<F: Fragment>(
    items: &mut [F],
    context: &F::Context,
) -> Result<Labels, Unsupported> {
    loop {
        let (mut labels, mut named, mut at) = (Labels::new(), HashMap::new(), 0i64);
        let mut starts = Vec::with_capacity(items.len());
        for item in items.iter() {
            if let Some(name) = item.label() {
                labels.push((name.to_owned(), at));
                named.insert(name.to_owned(), at);
            }
            starts.push(at);
            at += item.size(at, context) as i64;
        }
        let mut changed = false;
        for (item, &at) in items.iter_mut().zip(&starts) {
            changed |= item.relax(at, &named, context)?;
        }
        if !changed {
            return Ok(labels);
        }
    }
}

/// The padding of an `MCAlignFragment` at offset `at`: up to the multiple of
/// `1 << log2`, none where it takes more than `max_skip` (0: no limit).
pub fn align_padding(
    at: i64,
    log2: u8,
    max_skip: u8,
) -> usize {
    let pad = (at as u64).wrapping_neg() as usize & ((1usize << log2) - 1);
    if max_skip != 0 && pad > usize::from(max_skip) { 0 } else { pad }
}

/// `MCAssembler::relaxPrefAlign`: the alignment, as a power of two, a
/// `.prefalign` of `preferred` settles on at `at` when the body ends at `end`.
/// The body is what lies between the padding and its end, and the alignment
/// only grows from `computed`.
pub fn relax_pref_align(
    at: i64,
    computed: u8,
    preferred: u8,
    end: i64,
) -> u8 {
    let pad = (at as u64).wrapping_neg() as i64 & ((1i64 << computed) - 1);
    let body = end - (at + pad);
    let fit = (body.max(1) as u64).next_power_of_two().trailing_zeros() as u8;
    fit.min(preferred).max(computed)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 3-byte body at offset 1 with a 16-byte preference pads to 4 (the
    /// body's power of two), never to 16.
    /// A backend that says nothing of unwinding leaves a function to its DWARF
    /// frame: encoding 0, whatever its prologue states.
    #[test]
    fn a_backend_with_no_compact_unwind_encodes_zero() {
        struct Plain;
        impl AsmBackend for Plain {
            fn kind_info(
                &self,
                kind: FixupKind,
            ) -> FixupKindInfo {
                kind.info()
            }

            fn write_nop_data(
                &self,
                _: &mut Vec<u8>,
                _: usize,
            ) -> bool {
                false
            }
        }
        let prologue = [Cfi::DefCfaOffset { offset: 16 }, Cfi::Offset { register: 6, offset: -16 }];
        assert_eq!(Plain.generate_compact_unwind_encoding(&prologue), 0);
    }

    /// Made-up fragments, no target: a label, fixed bytes, a jump of 2 bytes
    /// that grows to 5 when its label is more than 127 bytes off.
    enum Test {
        Label(&'static str),
        Bytes(usize),
        Jump { to: &'static str, long: bool },
    }

    impl Fragment for Test {
        type Context = ();

        fn label(&self) -> Option<&str> {
            match self {
                Test::Label(name) => Some(name),
                _ => None,
            }
        }

        fn size(
            &self,
            _: i64,
            _: &(),
        ) -> usize {
            match self {
                Test::Label(_) => 0,
                Test::Bytes(count) => *count,
                Test::Jump { long, .. } => {
                    if *long {
                        5
                    } else {
                        2
                    }
                }
            }
        }

        fn relax(
            &mut self,
            at: i64,
            labels: &HashMap<String, i64>,
            _: &(),
        ) -> Result<bool, Unsupported> {
            let Test::Jump { to, long } = self else { return Ok(false) };
            let Some(target) = labels.get(*to) else { return Err(Unsupported(format!("{to} is nowhere"))) };
            if *long || (-128..=127).contains(&(target - (at + 2))) {
                return Ok(false);
            }
            *long = true;
            Ok(true)
        }
    }

    /// Lengthening one jump moved a label past the reach of another: the layout
    /// has to go round again, and stops at the smallest layout.
    #[test]
    fn a_layout_goes_round_until_no_jump_grows() {
        let mut items = vec![
            Test::Jump { to: "far", long: false },
            Test::Jump { to: "mid", long: false },
            Test::Label("mid"),
            Test::Bytes(124),
            Test::Jump { to: "mid", long: false },
            Test::Label("far"),
            Test::Bytes(1),
        ];
        let labels = layout(&mut items, &()).unwrap();
        // The first jump reaches `far` only at 2+2+124+2 = 130 > 127: long (5).
        // The second then lies at 5 and its label at 7, still short.
        assert_eq!(labels, vec![("mid".to_owned(), 7), ("far".to_owned(), 7 + 124 + 2)]);
        assert!(matches!(items[0], Test::Jump { long: true, .. }));
        assert!(matches!(items[1], Test::Jump { long: false, .. }));
    }

    /// An alignment pads to the multiple, and none at all where that is more
    /// than the most it may skip.
    #[test]
    fn an_alignment_pads_up_unless_it_may_skip_less() {
        assert_eq!(align_padding(5, 4, 0), 11);
        assert_eq!(align_padding(16, 4, 0), 0);
        assert_eq!(align_padding(5, 4, 10), 0);
        assert_eq!(align_padding(5, 4, 11), 11);
    }

    #[test]
    fn a_jump_to_nowhere_is_refused() {
        let mut items = vec![Test::Jump { to: "gone", long: false }];
        assert!(layout(&mut items, &()).unwrap_err().0.contains("nowhere"));
    }

    #[test]
    fn a_prefalign_grows_to_the_body_and_no_further() {
        assert_eq!(relax_pref_align(1, 0, 4, 1 + 3), 2);
        assert_eq!(relax_pref_align(0, 0, 4, 40), 4);
        assert_eq!(relax_pref_align(5, 3, 1, 9), 3);
    }
}
