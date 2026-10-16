use crate::module::{BlockId, Function, InstId, Module, Operand, ValueId};
use crate::opcode::{BinaryOp, Flags, Opcode};
use crate::{Position, parse};

const TEXT: &str = "define i16 @f(i16 %a, i1 %c, ptr %p) {
entry:
  %x = add i16 %a, 1
  %l = load i16, ptr %p
  store i16 %x, ptr %p
  br i1 %c, label %yes, label %no
yes:
  %y = mul i16 %l, 2
  br label %join
no:
  %n = sub i16 %x, 3
  br label %join
join:
  %r = phi i16 [ %y, %yes ], [ %n, %no ]
  %s = add i16 %r, %x
  ret i16 %s
}
";

fn module() -> Module {
    parse::module(TEXT).unwrap_or_else(|error| panic!("{error}"))
}

/// The same pseudo-random numbers on every run.
struct Dice(u64);

impl Dice {
    fn below(
        &mut self,
        n: usize,
    ) -> usize {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((self.0 >> 33) as usize) % n.max(1)
    }
}

/// One edit chosen by the dice: any mutator, with an operand or a place that
/// may make no sense to a verifier (the arenas do not care), skipped where the
/// editor refuses it.
fn edit(
    f: &mut Function,
    dice: &mut Dice,
    insts: &mut Vec<InstId>,
    blocks: &mut Vec<BlockId>,
) {
    // Only what stands in a block: the editor does not take an edit of an
    // erased instruction.
    let live: Vec<InstId> = insts.iter().copied().filter(|&one| f.parent(one).is_some()).collect();
    let inst = live[dice.below(live.len())];
    let other = live[dice.below(live.len())];
    let values: Vec<ValueId> = live.iter().filter_map(|&one| f.instruction(one).result).collect();
    let value = values[dice.below(values.len())];
    let with = if dice.below(3) == 0 {
        Operand::Value(f.parameters()[0])
    } else {
        Operand::Value(values[dice.below(values.len())])
    };
    match dice.below(15) {
        0 => {
            let ty = f.instruction(inst).ty;
            let made = f.create_instruction(
                Opcode::Binary(BinaryOp::Add),
                ty,
                vec![with, Operand::Value(value)],
                Flags::default(),
                None,
            );
            insts.push(made);
            let _ = f.insert(made, Position::Before(other));
        }
        1 => {
            let _ = f.move_to(inst, Position::Before(other));
        }
        2 => {
            let at = f.instruction(inst).operands.len();
            if at > 0 {
                let index = dice.below(at);
                f.set_operand(inst, index, with);
            }
        }
        3 => {
            let mut operands = f.instruction(inst).operands.clone();
            if !operands.is_empty() {
                let index = dice.below(operands.len());
                operands[index] = with;
                f.set_operands(inst, operands);
            }
        }
        4 => f.replace_all_uses_with(value, with),
        5 => f.replace_value(value, with),
        6 => {
            let _ = f.erase(inst);
        }
        7 => {
            let _ = f.erase_all(&[inst, other]);
        }
        8 => {
            let made = f.create_block(Some("spec"));
            blocks.push(made);
            let after = f.layout()[dice.below(f.layout().len())];
            let _ = f.insert_block(made, Some(after));
        }
        9 => {
            let block = blocks[dice.below(blocks.len())];
            let _ = f.erase_block(block);
        }
        10 => f.set_flags(inst, Flags::default()),
        11 => {
            let copy = f.clone_instruction(inst);
            insts.push(copy);
            let _ = f.insert(copy, Position::Before(other));
        }
        12 => f.make_store_volatile(inst),
        13 => f.add_debug_record(inst, crate::module::MetadataId(1), crate::module::DebugWhat::Value(with)),
        _ => {
            let last = f.layout()[f.layout().len() - 1];
            let run: Vec<InstId> = f.block(f.layout()[0]).instructions().iter().copied().take(2).collect();
            let _ = f.move_run(&run, last);
        }
    }
}

/// A rejected speculation left the function other than it was (its use lists
/// in another order, a block layout, an erased id back): pricing a hoist or a
/// numbering on the live function would then have kept the experiment.
#[test]
fn a_rollback_gives_back_an_equal_function_after_any_edits() {
    for seed in 0..200 {
        let mut module = module();
        let f = module.function_mut("f").expect("@f").1;
        let mut dice = Dice(seed);
        let mut insts: Vec<InstId> = f.walk().map(|(_, inst)| inst).collect();
        let mut blocks: Vec<BlockId> = f.layout().to_vec();
        let mut open = Vec::new();
        for _ in 0..40 {
            match dice.below(8) {
                0 => {
                    let at = f.mark();
                    open.push((f.clone(), f.checkpoint(), at, insts.len(), blocks.len()));
                }
                1 if !open.is_empty() => {
                    let (was, checkpoint, at, made, made_blocks) = open.pop().expect("open");
                    if dice.below(2) == 0 {
                        // Set aside, then found to be the one to keep.
                        let after = f.clone();
                        let logged = f.changes_since(at).map(<[_]>::len);
                        let redo = f.rollback_keeping(checkpoint);
                        assert_eq!(*f, was, "seed {seed}");
                        f.redo(redo);
                        assert_eq!(*f, after, "seed {seed}: made again");
                        assert!(f.check_uses().is_empty(), "seed {seed}");
                        assert_eq!(f.changes_since(at).map(<[_]>::len), logged, "seed {seed}: the log came back");
                        continue;
                    }
                    f.rollback(checkpoint);
                    insts.truncate(made);
                    blocks.truncate(made_blocks);
                    assert_eq!(*f, was, "seed {seed}");
                    assert!(f.check_uses().is_empty(), "seed {seed}: {:?}", f.check_uses());
                    assert_eq!(f.changes_since(at).map(<[_]>::len), Some(0), "seed {seed}: the log was cut");
                }
                _ => {
                    edit(f, &mut dice, &mut insts, &mut blocks);
                    assert!(f.check_uses().is_empty(), "seed {seed}: {:?}", f.check_uses());
                }
            }
        }
        while let Some((was, checkpoint, _, made, made_blocks)) = open.pop() {
            f.rollback(checkpoint);
            insts.truncate(made);
            blocks.truncate(made_blocks);
            assert_eq!(*f, was, "seed {seed}");
        }
    }
}

/// A mark taken during a speculation resolved after its rollback and a new
/// edit: an analysis computed inside it was then brought up to date from a
/// history that never happened.
#[test]
fn a_mark_taken_in_a_speculation_stops_resolving_after_its_rollback() {
    let mut module = module();
    let f = module.function_mut("f").expect("@f").1;
    let (x, _) = f
        .walk()
        .find_map(|(_, inst)| {
            f.instruction(inst).result.filter(|&r| f.value(r).name.as_deref() == Some("x")).map(|r| (inst, r))
        })
        .expect("%x");
    let before = f.mark();
    let checkpoint = f.checkpoint();
    f.set_flags(x, Flags::default());
    let inside = f.mark();
    assert!(f.changes_since(inside).is_some());
    f.rollback(checkpoint);
    f.set_flags(x, Flags::default());
    f.set_flags(x, Flags::default());
    assert!(f.changes_since(inside).is_none(), "a point of the undone history");
    assert_eq!(f.changes_since(before).map(<[_]>::len), Some(2));
}

/// Released, the speculation stays: the function is as edited and the log
/// holds what was done.
#[test]
fn a_released_checkpoint_keeps_the_edits_and_their_log() {
    let mut module = module();
    let f = module.function_mut("f").expect("@f").1;
    let (_, inst) = f.walk().next().expect("an instruction");
    let mark = f.mark();
    let checkpoint = f.checkpoint();
    f.set_flags(inst, Flags::default());
    f.release(checkpoint);
    assert_eq!(f.changes_since(mark).map(<[_]>::len), Some(1));
}

/// The edits that change more than a part (parameters, the whole body, the
/// metadata numbers) are rolled back too: the signature then stayed a
/// speculation's.
#[test]
fn a_rollback_undoes_the_edits_that_change_the_whole_function() {
    let mut module = module();
    let (context, f) = module.function_mut("f").expect("@f");
    let was = f.clone();
    let checkpoint = f.checkpoint();
    let i16 = context.types.int(16);
    f.insert_parameters(context, 1, &[i16]);
    f.renumber_metadata(&|one| crate::module::MetadataId(one.0 + 1));
    f.delete_body();
    assert_ne!(*f, was);
    f.rollback(checkpoint);
    assert_eq!(*f, was);
    assert!(f.check_uses().is_empty());
}
