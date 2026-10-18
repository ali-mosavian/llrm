//! `MemorySSAUpdater` against a graph made afresh: after each edit of the body
//! and the updater's matching call, the updated graph says what `Mssa::build`
//! says of the body as it stands.

use llrm_mir::datalayout::DataLayout;
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Module};
use llrm_mir::opcode::Opcode;

use super::build::Effect;
use super::edit::Where;
use super::updater::{MemorySsaUpdater, MoveTo};
use super::{Kind, Mssa, shadow};
use crate::cfg::Shape;
use crate::memory::Unit;
use crate::memoryssa::Accesses;
use crate::testing::{DOS, function, layout, parsed};

struct Body {
    module: Module,
    layout: DataLayout,
}

impl Body {
    fn new(text: &str) -> Self {
        let module = parsed(&format!(
            "{DOS}@ga = global i16 0\n@gb = global i16 0\n@gc = global i16 0\ndeclare void @anything()\n\n{text}"
        ));
        let layout = layout(&module);
        Self { module, layout }
    }

    fn function(&self) -> &Function {
        function(&self.module, "f")
    }

    fn mutate<R>(
        &mut self,
        edit: impl FnOnce(&mut Function) -> R,
    ) -> R {
        let (_, function) = self.module.function_mut("f").expect("@f");
        edit(function)
    }

    /// `f` given what the body's instructions do to memory, as it stands.
    fn with_effects<R>(
        &self,
        f: impl FnOnce(&dyn Fn(InstId) -> Effect) -> R,
    ) -> R {
        let unit = Unit::of(&self.module, &self.layout, self.function());
        let accesses = Accesses::plain(&unit, &crate::consts::Calls::default());
        let effect = shadow::effect_of(&accesses, self.function());
        f(&effect)
    }

    fn fresh(&self) -> Mssa {
        let shape = Shape::of(self.function());
        self.with_effects(|effect| Mssa::build(self.function(), &shape.dominance, effect))
    }

    /// The graph says what a fresh one says of the body.
    fn check(
        &self,
        mssa: &Mssa,
        what: &str,
    ) {
        let shape = Shape::of(self.function());
        self.with_effects(|effect| {
            shadow::same_as_fresh(mssa, self.function(), &shape.dominance, effect)
                .unwrap_or_else(|why| panic!("after {what}: {why}\n{}", llrm_mir::print::module(&self.module)))
        });
    }

    fn block(
        &self,
        name: &str,
    ) -> BlockId {
        self.function()
            .layout()
            .iter()
            .copied()
            .find(|&b| self.function().block(b).name.as_deref() == Some(name))
            .unwrap_or_else(|| panic!("no {name}"))
    }

    fn memory_instructions(&self) -> Vec<InstId> {
        self.function()
            .walk()
            .map(|(_, inst)| inst)
            .filter(|&inst| {
                matches!(
                    self.function().instruction(inst).opcode,
                    Opcode::Load { .. } | Opcode::Store { .. }
                )
            })
            .collect()
    }

    /// `inst` taken out: the access leaves first, as `eraseInstruction` does.
    fn erase(
        &mut self,
        mssa: &mut Mssa,
        updater: &mut MemorySsaUpdater,
        inst: InstId,
    ) {
        let access = mssa.get_memory_access(inst).expect("an access");
        updater.remove_memory_access(mssa, self.function(), access, true);
        self.mutate(|function| function.erase(inst)).expect("erased");
        self.check(mssa, &format!("erasing {inst:?}"));
    }

    /// `inst` moved to `block`, before the instruction at `index` of it (the
    /// terminator when `index` is past the others).
    fn move_to(
        &mut self,
        mssa: &mut Mssa,
        updater: &mut MemorySsaUpdater,
        inst: InstId,
        block: BlockId,
        index: usize,
    ) {
        let access = mssa.get_memory_access(inst).expect("an access");
        self.mutate(|function| {
            let others: Vec<InstId> =
                function.block(block).instructions().iter().copied().filter(|&one| one != inst).collect();
            let before = others[index.min(others.len() - 1)];
            function.move_to(inst, Position::Before(before)).expect("moved");
        });
        let place = mssa.place_before_next_access(self.function(), block, Some(inst));
        match place {
            Where::Before(next) => updater.move_before(mssa, self.function(), access, next),
            Where::End => updater.move_to_place(mssa, self.function(), access, block, MoveTo::End, None),
        }
        self.check(mssa, &format!("moving {inst:?} to {block:?}:{index}"));
    }

    /// A copy of `like` placed in `block` before the instruction at `index`,
    /// its access made and put in.
    fn insert_copy(
        &mut self,
        mssa: &mut Mssa,
        updater: &mut MemorySsaUpdater,
        like: InstId,
        block: BlockId,
        index: usize,
    ) -> InstId {
        let copy = self.mutate(|function| {
            let copy = function.clone_instruction(like);
            let before =
                function.block(block).instructions()[index.min(function.block(block).instructions().len() - 1)];
            function.insert(copy, Position::Before(before)).expect("inserted");
            copy
        });
        let place = mssa.place_before_next_access(self.function(), block, Some(copy));
        let access = self
            .with_effects(|effect| mssa.create_defined_access(block, copy, None, effect))
            .expect("a memory instruction has an access");
        mssa.insert_into_lists_before(access, block, place);
        if mssa.access(access).kind == Kind::Def {
            updater.insert_def(mssa, self.function(), access, true);
        } else {
            updater.insert_use(mssa, self.function(), access, true);
        }
        self.check(mssa, &format!("inserting a copy of {like:?} at {block:?}:{index}"));
        copy
    }
}

const LOOP: &str = "define i16 @f(i1 %c) {
b0:
  store i16 1, ptr @ga
  br label %b1

b1:
  %x = load i16, ptr @ga
  store i16 2, ptr @gb
  %y = load i16, ptr @gc
  br i1 %c, label %b1, label %b2

b2:
  %z = load i16, ptr @gb
  ret i16 %z
}
";

fn find(
    body: &Body,
    block: &str,
    at: usize,
) -> InstId {
    body.function().block(body.block(block)).instructions()[at]
}

/// A store the loop does not read leaves it for the preheader: the loads after
/// it read it, and the loop's phi stays only where a store of the loop remains.
#[test]
fn test_a_store_moved_out_of_a_loop_is_the_def_the_loop_reads() {
    let mut body = Body::new(LOOP);
    let mut mssa = body.fresh();
    let mut updater = MemorySsaUpdater::new();
    let store = find(&body, "b1", 1);
    let to = body.block("b0");
    body.move_to(&mut mssa, &mut updater, store, to, 99);
}

/// A store erased: what read it reads what it read.
#[test]
fn test_an_erased_store_is_read_through() {
    let mut body = Body::new(LOOP);
    let mut mssa = body.fresh();
    let mut updater = MemorySsaUpdater::new();
    let store = find(&body, "b1", 1);
    body.erase(&mut mssa, &mut updater, store);
}

/// A load moved after the loop and a copy of a store put in it.
#[test]
fn test_a_load_moved_to_the_exit_and_a_store_put_in_the_loop() {
    let mut body = Body::new(LOOP);
    let mut mssa = body.fresh();
    let mut updater = MemorySsaUpdater::new();
    let load = find(&body, "b1", 2);
    let exit = body.block("b2");
    body.move_to(&mut mssa, &mut updater, load, exit, 0);
    let store = find(&body, "b0", 0);
    let b1 = body.block("b1");
    body.insert_copy(&mut mssa, &mut updater, store, b1, 0);
}

/// Cells the body reads and writes at random places: random bodies of up to 7
/// blocks with back edges and joins, then up to 10 edits of them at random, the
/// graph checked after each.
#[test]
fn test_random_edits_leave_the_graph_a_fresh_one_would_be() {
    struct Rng(u64);
    impl Rng {
        fn next(
            &mut self,
            n: usize,
        ) -> usize {
            self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            ((self.0 >> 33) as usize) % n
        }
    }
    for seed in 0..400u64 {
        let mut rng = Rng(seed * 7919 + 1);
        let blocks = 3 + rng.next(5);
        let mut text = String::from("define i16 @f(i1 %c) {\n");
        let mut name = 0;
        for b in 0..blocks {
            text += &format!("b{b}:\n");
            for _ in 0..(rng.next(4)) {
                let cell = ["ga", "gb", "gc"][rng.next(3)];
                match rng.next(3) {
                    0 => text += &format!("  store i16 {}, ptr @{cell}\n", rng.next(9)),
                    1 => {
                        text += &format!("  %v{name} = load i16, ptr @{cell}\n");
                        name += 1;
                    }
                    _ => text += "  call void @anything()\n",
                }
            }
            text += &if b + 1 == blocks {
                "  ret i16 0\n".to_owned()
            } else if rng.next(3) == 0 {
                format!("  br label %b{}\n", b + 1)
            } else {
                // The next block is always one way on; the other way is a join
                // further on or a back edge to a block at or
                // before this one. (the entry block has no
                // predecessor, as in LLVM IR).
                let other = if b > 0 && rng.next(2) == 0 { 1 + rng.next(b) } else { b + 1 + rng.next(blocks - b - 1) };
                format!("  br i1 %c, label %b{}, label %b{other}\n", b + 1)
            };
            text += "\n";
        }
        text += "}\n";
        // Every block must be reachable from the entry: a forward edge to the
        // next block is always there.
        let mut body = Body::new(&text);
        // Non-minimal phis in cycles are left by the updater as by LLVM's (its
        // own comment): irreducible control flow is not compared.
        if !Shape::of(body.function()).dominance.irreducible(body.function()).is_empty() {
            continue;
        }
        let mut mssa = body.fresh();
        let mut updater = MemorySsaUpdater::new();
        for _step in 0..10 {
            let memory = body.memory_instructions();
            if memory.is_empty() {
                break;
            }
            let layout_blocks: Vec<BlockId> = body.function().layout().to_vec();
            let inst = memory[rng.next(memory.len())];
            let block = layout_blocks[rng.next(layout_blocks.len())];
            let index = rng.next(4);
            let op = rng.next(3);
            match op {
                0 if memory.len() > 1 => body.erase(&mut mssa, &mut updater, inst),
                1 => {
                    // A move needs somewhere to go before: the block has its
                    // terminator.
                    body.move_to(&mut mssa, &mut updater, inst, block, index);
                }
                _ => {
                    body.insert_copy(&mut mssa, &mut updater, inst, block, index);
                }
            }
        }
    }
}
