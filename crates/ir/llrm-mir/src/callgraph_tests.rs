use crate::callgraph::CallGraph;
use crate::parse;

fn id(
    module: &crate::Module,
    name: &str,
) -> crate::GlobalId {
    module.functions().find(|(_, global, _)| global.name.as_deref() == Some(name)).map(|(id, _, _)| id).expect(name)
}

const CALLS: &str = "define void @leaf() {
b0:
  ret void
}

define void @listed(ptr %p) {
b0:
  call void %p(), !callees !0
  ret void
}

define void @unlisted(ptr %p) {
b0:
  call void %p()
  ret void
}

define void @direct() {
b0:
  call void @leaf()
  ret void
}

!0 = !{ptr @leaf, ptr @direct}
";

/// An indirect call was dropped from the graph: a function reaching itself
/// through a pointer was no cycle, and a function calling anything was
/// indistinguishable from one calling nothing.
#[test]
fn an_indirect_call_reaches_what_callees_lists_and_otherwise_any() {
    let module = parse::module(CALLS).expect("parses");
    let graph = CallGraph::new(&module);
    let (leaf, listed, unlisted, direct) =
        (id(&module, "leaf"), id(&module, "listed"), id(&module, "unlisted"), id(&module, "direct"));
    assert!(graph.reaches(listed, leaf) && graph.reaches(listed, direct), "its listed callees");
    assert!(!graph.calls_unknown(listed), "a list bounds the call");
    assert!(graph.calls_unknown(unlisted), "no list: any function");
    assert!(!graph.reaches(unlisted, leaf), "an unknown call names no edge");
    assert!(!graph.calls_unknown(direct) && !graph.calls_unknown(leaf));
}

fn edges(list: &[(u32, u32)]) -> CallGraph<u32> {
    let mut callees = std::collections::BTreeMap::<u32, std::collections::BTreeSet<u32>>::new();
    for &(from, to) in list {
        callees.entry(from).or_default().insert(to);
        callees.entry(to).or_default();
    }
    CallGraph::from_edges(callees)
}

/// One answer to "is it in a cycle": agrees with a walk from each node to itself,
/// on a self-call, a cycle, a diamond and cycles that touch.
#[test]
fn test_recursive_is_the_components_answer_and_agrees_with_walking() {
    let graphs: [&[(u32, u32)]; 5] = [
        &[(0, 0)],
        &[(0, 1), (1, 2), (2, 0), (2, 3)],
        &[(0, 1), (0, 2), (1, 3), (2, 3)],
        &[(0, 1), (1, 0), (1, 2), (2, 3), (3, 2), (3, 4)],
        &[(0, 1), (1, 2), (2, 1), (2, 2), (0, 3)],
    ];
    for list in graphs {
        let graph = edges(list);
        for node in 0..5 {
            assert_eq!(graph.recursive(node), graph.reaches(node, node), "{list:?} node {node}");
        }
    }
    let ring = edges(&[(0, 1), (1, 2), (2, 0), (3, 3)]);
    assert!(ring.together(0, 2) && !ring.together(0, 3) && ring.recursive(3));
}

/// A function a call's `!callees` lists may be entered by an indirect call, so its address is
/// taken though every direct call of it names it as a callee: `@leaf` is called by `@direct` and
/// listed, `@listed` is neither.
#[test]
fn a_function_a_callees_list_names_has_its_address_taken() {
    let module = parse::module(CALLS).expect("parses");
    let named = crate::callgraph::addressed(&module);
    assert!(named.contains(&id(&module, "leaf")) && named.contains(&id(&module, "direct")));
    assert!(!named.contains(&id(&module, "listed")) && !named.contains(&id(&module, "unlisted")));
}

/// Only an internal function never named but as a callee is reached by direct calls alone: an
/// external one is called from other modules, an addressed one through its pointer.
#[test]
fn direct_only_is_internal_and_never_addressed() {
    let module = parse::module(
        "define internal void @only() {
b0:
  ret void
}

define void @exported() {
b0:
  call void @only()
  ret void
}

define internal void @taken() {
b0:
  ret void
}

@table = global ptr @taken
",
    )
    .expect("parses");
    let found = crate::callgraph::direct_only(&module);
    assert!(found.contains(&id(&module, "only")));
    assert!(!found.contains(&id(&module, "exported")) && !found.contains(&id(&module, "taken")));
}
