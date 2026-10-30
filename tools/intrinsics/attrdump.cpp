// Prints the attributes LLVM attaches to every intrinsic: "<name> <index> <attribute>" per attribute, index
// f (function), r (return) or the parameter's number. Types are i32 throughout, so ranges print at i32.
// Built and run by test_gen.py against the LLVM build of the pinned commit.
#include "llvm/IR/Attributes.h"
#include "llvm/IR/DerivedTypes.h"
#include "llvm/IR/Intrinsics.h"
#include "llvm/IR/LLVMContext.h"
#include <cstdio>
#include <vector>

using namespace llvm;

static void dump(const char *name, const char *index, AttributeSet set) {
  for (Attribute attr : set) std::printf("%s %s %s\n", name, index, attr.getAsString().c_str());
}

int main() {
  LLVMContext context;
  Type *i32 = Type::getInt32Ty(context);
  const unsigned parameters = 40;
  FunctionType *type = FunctionType::get(i32, std::vector<Type *>(parameters, i32), false);
  for (unsigned id = 1; id < Intrinsic::num_intrinsics; ++id) {
    std::string name = Intrinsic::getBaseName(id).str();
    AttributeList list = Intrinsic::getAttributes(context, id, type);
    dump(name.c_str(), "f", list.getFnAttrs());
    dump(name.c_str(), "r", list.getRetAttrs());
    for (unsigned at = 0; at < parameters; ++at) dump(name.c_str(), std::to_string(at + 1).c_str(), list.getParamAttrs(at));
  }
}
