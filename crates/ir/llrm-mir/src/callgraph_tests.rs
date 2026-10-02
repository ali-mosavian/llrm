use crate::callgraph::CallGraph;
use crate::parse;

fn id(module: &crate::Module, name: &str) -> crate::GlobalId {
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
    let (leaf, listed, unlisted, direct) = (id(&module, "leaf"), id(&module, "listed"), id(&module, "unlisted"), id(&module, "direct"));
    assert!(graph.reaches(listed, leaf) && graph.reaches(listed, direct), "its listed callees");
    assert!(!graph.calls_unknown(listed), "a list bounds the call");
    assert!(graph.calls_unknown(unlisted), "no list: any function");
    assert!(!graph.reaches(unlisted, leaf), "an unknown call names no edge");
    assert!(!graph.calls_unknown(direct) && !graph.calls_unknown(leaf));
}
