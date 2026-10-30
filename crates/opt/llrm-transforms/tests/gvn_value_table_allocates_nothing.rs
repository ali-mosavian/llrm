//! `ValueTable::lookup_or_add` took 724 instructions a call, most of them
//! allocation: a `Vec` of operand numbers for each expression, a list for each
//! leader. A table that has numbered one function numbers the next with no
//! allocation at all.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

use llrm_mir::module::Operand;
use llrm_mir::passes::Outer;
use llrm_transforms::gvn::table::{Env, LeaderMap, ValueTable};

struct Counting;

thread_local! {
    /// This thread's allocations alone; a `const` cell with no destructor
    /// allocates nothing to be read.
    static ALLOCATED: Cell<usize> = const { Cell::new(0) };
}

fn allocated() -> usize {
    ALLOCATED.with(Cell::get)
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(
        &self,
        layout: Layout,
    ) -> *mut u8 {
        ALLOCATED.with(|count| count.set(count.get() + 1));
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(
        &self,
        pointer: *mut u8,
        layout: Layout,
    ) {
        unsafe { System.dealloc(pointer, layout) }
    }
}

#[global_allocator]
static COUNTING: Counting = Counting;

#[test]
fn test_a_table_that_numbered_one_function_numbers_the_next_with_no_allocation() {
    let mut text = String::from("define i32 @f(i32 %a, i32 %b) {\nentry:\n");
    for k in 0..40 {
        text += &format!("  %x{k} = add i32 %a, {k}\n  %y{k} = mul i32 %x{k}, %b\n");
    }
    text += "  ret i32 %y39\n}\n";
    let mut module = llrm_mir::parse::module(&text).unwrap_or_else(|why| panic!("{why:?}"));
    let outer = Outer::of(&module, None);
    let layout = llrm_mir::datalayout::DataLayout::default();
    let function = module.named("f").and_then(|id| module.global(id).function()).expect("@f").clone();
    let values: Vec<Operand> =
        function.walk().filter_map(|(_, inst)| function.instruction(inst).result.map(Operand::Value)).collect();
    let mut table = ValueTable::new();
    let mut leaders = LeaderMap::default();
    let mut run = |table: &mut ValueTable, leaders: &mut LeaderMap| {
        let mut env = Env { context: &mut module.context, function: &function, layout: &layout, outer: &outer };
        for &value in &values {
            let number = table.lookup_or_add(&mut env, value);
            leaders.insert(number, value, function.entry().expect("an entry"));
        }
    };
    run(&mut table, &mut leaders);
    table.clear();
    leaders.clear();
    let before = allocated();
    run(&mut table, &mut leaders);
    let made = allocated() - before;
    assert_eq!(made, 0, "numbering {} values again took {made} allocations", values.len());
}
