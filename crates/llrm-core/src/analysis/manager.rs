//! Analyses of a body, kept while one transaction runs: LLVM's
//! `AnalysisManager` (llvm/include/llvm/IR/PassManager.h). No invalidation is
//! needed, since a pass that changes a body makes a new one; holding the old
//! keeps its address from being reused for another.

use std::any::{Any, TypeId};
use std::cell::RefCell;
use std::rc::Rc;

use crate::model::mir::MirBody;
use crate::support::hash::HashMap;

type Results = HashMap<(TypeId, usize), Vec<(Box<dyn Any>, Rc<MirBody>, Rc<dyn Any>)>>;

thread_local! {
    static RESULTS: RefCell<Option<Results>> = const { RefCell::new(None) };
}

/// Run `inside` with analyses shared between its passes.
pub fn scoped<T>(inside: impl FnOnce() -> T) -> T {
    let outer = RESULTS.with(|results| results.replace(Some(Results::default())));
    let result = inside();
    RESULTS.with(|results| *results.borrow_mut() = outer);
    result
}

/// `compute`'s answer for `body` in context `key`, found once per scope.
pub fn cached<K: Eq + 'static, R: 'static>(body: &Rc<MirBody>, key: K, compute: impl FnOnce() -> R) -> Rc<R> {
    let slot = (TypeId::of::<R>(), Rc::as_ptr(body) as usize);
    let saved = RESULTS.with(|results| {
        results.borrow().as_ref().and_then(|results| {
            results.get(&slot)?.iter().find(|(saved, ..)| saved.downcast_ref::<K>() == Some(&key)).map(|(.., result)| Rc::clone(result))
        })
    });
    llrm_support::debug!("manager", "{} {}", std::any::type_name::<R>().rsplit("::").next().unwrap_or(""), if saved.is_some() { "hit" } else { "miss" });
    if let Some(saved) = saved {
        return saved.downcast::<R>().expect("a result keeps its type");
    }
    let result = Rc::new(compute());
    RESULTS.with(|results| {
        if let Some(results) = results.borrow_mut().as_mut() {
            results.entry(slot).or_default().push((Box::new(key), Rc::clone(body), Rc::clone(&result) as Rc<dyn Any>));
        }
    });
    result
}
