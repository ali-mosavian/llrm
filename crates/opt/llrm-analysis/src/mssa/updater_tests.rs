//! `MemorySSAUpdater` against a graph made afresh: after each edit of the body
//! and the updater's matching call, the updated graph says what `Mssa::build`
//! says of the body as it stands.

use llrm_mir::datalayout::DataLayout;
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Module};

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
        let unit = crate::testing::with_registers(Unit::of(&self.module, &self.layout, self.function()));
        let accesses = Accesses::plain(&unit, &crate::consts::Calls::default());
        let effect = shadow::effect_of(&accesses, self.function());
        f(&effect)
    }

    fn fresh(&self) -> Mssa {
        let shape = Shape::of(self.function());
        self.with_effects(|effect| Mssa::build(self.function(), &shape.dominance, effect))
    }

    /// The graph says what a fresh one says of the body, both asked of the
    /// walker.
    fn check(
        &self,
        mssa: &mut Mssa,
        what: &str,
    ) {
        let shape = Shape::of(self.function());
        let unit = crate::testing::with_registers(Unit::of(&self.module, &self.layout, self.function()));
        let accesses = Accesses::plain(&unit, &crate::consts::Calls::default());
        let aa = super::alias::AliasQuery::new(&unit, &accesses);
        let effect = shadow::effect_of(&accesses, self.function());
        shadow::same_as_fresh(mssa, self.function(), &shape.dominance, &effect, &aa)
            .unwrap_or_else(|why| panic!("after {what}: {why}\n{}", llrm_mir::print::module(&self.module)));
    }

    /// The walker's answers on the updated graph (its cache kept from before
    /// the edit) are those on a fresh one.
    fn check_walks(
        &self,
        mssa: &mut Mssa,
        what: &str,
    ) {
        let unit = crate::testing::with_registers(Unit::of(&self.module, &self.layout, self.function()));
        let accesses = Accesses::plain(&unit, &crate::consts::Calls::default());
        let aa = super::alias::AliasQuery::new(&unit, &accesses);
        let mut fresh = self.fresh();
        let (updated, expected) =
            (shadow::walked(mssa, &aa, self.function()), shadow::walked(&mut fresh, &aa, self.function()));
        let differ: Vec<_> = updated
            .iter()
            .filter(|(inst, name)| expected.get(inst) != Some(name))
            .map(|(inst, name)| format!("{inst:?}: updated {name:?}, fresh {:?}", expected.get(&inst)))
            .collect();
        assert!(
            differ.is_empty(),
            "after {what}: the walker's answers differ: {differ:?}\n{}\nupdated:\n{}\nfresh:\n{}",
            llrm_mir::print::module(&self.module),
            self.dump(mssa),
            self.dump(&fresh)
        );
    }

    fn dump(
        &self,
        mssa: &Mssa,
    ) -> String {
        let mut out = String::new();
        for &block in self.function().layout() {
            out += &format!("{}:", self.function().block(block).name.clone().unwrap_or_default());
            for &access in mssa.get_block_accesses(block).unwrap_or(&[]) {
                let one = mssa.access(access);
                out += &format!(
                    " [{access} {:?} inst={:?} def={:?} opt={:?} in={:?}]",
                    one.kind, one.inst, one.defining, one.optimized, one.incoming
                );
            }
            out += "\n";
        }
        out
    }

    /// Asks the walker of every load and store, so that its cache is filled.
    fn warm(
        &self,
        mssa: &mut Mssa,
    ) {
        let unit = crate::testing::with_registers(Unit::of(&self.module, &self.layout, self.function()));
        let accesses = Accesses::plain(&unit, &crate::consts::Calls::default());
        let aa = super::alias::AliasQuery::new(&unit, &accesses);
        shadow::walked(mssa, &aa, self.function());
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
        self.check_walks(mssa, &format!("erasing {inst:?}"));
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
            let others: Vec<InstId> = function.iter_block(block).filter(|&one| one != inst).collect();
            let before = others[index.min(others.len() - 1)];
            function.move_to(inst, Position::Before(before)).expect("moved");
        });
        let place = mssa.place_before_next_access(self.function(), block, Some(inst));
        match place {
            Where::Before(next) => updater.move_before(mssa, self.function(), access, next),
            Where::End => updater.move_to_place(mssa, self.function(), access, block, MoveTo::End, None),
        }
        self.check_walks(mssa, &format!("moving {inst:?} to {block:?}:{index}"));
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
            let before = function.iter_block(block).nth(index.min(function.block(block).len() - 1)).unwrap();
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
        self.check_walks(mssa, &format!("inserting a copy of {like:?} at {block:?}:{index}"));
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
    body.function().iter_block(body.block(block)).nth(at).unwrap()
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

/// A store put in a block between a store and the later one whose cached
/// clobber it is. LLVM's `insertDef` renames the uses it dominates, so a later
/// store in another block keeps the first as its clobber past a write of the
/// same cell (the DEVIATE `reset_optimized_defs` closes).
#[test]
fn test_a_def_inserted_between_a_def_and_its_cached_clobber_is_the_clobber() {
    let mut body = Body::new(
        "define void @f() {
b0:
  store i16 1, ptr @gb
  br label %b1

b1:
  store i16 2, ptr @ga
  br label %b2

b2:
  store i16 3, ptr @gb
  ret void
}
",
    );
    let mut mssa = body.fresh();
    let mut updater = MemorySsaUpdater::new();
    body.warm(&mut mssa);
    let (first, last) = (find(&body, "b0", 0), find(&body, "b2", 0));
    let copy = body.insert_copy(&mut mssa, &mut updater, first, body.block("b1"), 0);
    body.warm(&mut mssa);
    let (copy, last) = (mssa.get_memory_access(copy), mssa.get_memory_access(last).expect("an access"));
    assert_eq!(mssa.access(last).optimized, copy, "the last store reads the new one, not the first");
}
