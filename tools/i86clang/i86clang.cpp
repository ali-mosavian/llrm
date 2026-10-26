// i86clang: clang's front end and IR generator for llrm's 16-bit x86 (m16), with Borland's memory models.
//
// A TargetInfo of its own (16-bit int, 16/32-bit pointers) under clang's own CodeGenerator; the unoptimised IR goes to stdout and
// llrm-llc optimises and selects it. A model fixes the default distance of code and of data separately:
//
//            code   data
//   small    near   near
//   medium   far    near      (llrm's own default)
//   compact  near   far
//   large    far    far
//   huge     far    far       (+ objects over 64K when declared huge)
//
// Far code is the datalayout's program address space (P1): clang itself puts every function in address space 1. A pointer is
// spelled with its own distance wherever the source does not: a data pointer by an address space (near 0, far 1, huge 3), a
// function pointer by Microsoft's pointer-size qualifier (`__uptr` near, `__ptr64` far; clang gives them no other meaning), both
// by a first parse that writes the model's default (`annotate`). Those qualifiers act only when a default pointer is 32 bits,
// so it is, and nothing is left to it: every pointer in the source says its distance. Objects stay in address space 0.
#include <algorithm>
#include <map>
#include <memory>
#include <set>
#include <string>
#include <vector>

#include "clang/AST/ASTConsumer.h"
#include "clang/AST/RecursiveASTVisitor.h"
#include "clang/AST/TypeLoc.h"
#include "clang/Basic/CodeGenOptions.h"
#include "clang/Basic/MacroBuilder.h"
#include "clang/Basic/TargetInfo.h"
#include "clang/CodeGen/ModuleBuilder.h"
#include "clang/Frontend/CompilerInstance.h"
#include "clang/Frontend/FrontendAction.h"
#include "clang/Lex/Lexer.h"
#include "clang/Tooling/CommonOptionsParser.h"
#include "clang/Tooling/Tooling.h"
#include "llvm/ADT/STLExtras.h"
#include "llvm/IR/Constants.h"
#include "llvm/IR/IRBuilder.h"
#include "llvm/IR/Instructions.h"
#include "llvm/IR/LLVMContext.h"
#include "llvm/IR/Module.h"
#include "llvm/Support/CommandLine.h"
#include "llvm/Support/MemoryBuffer.h"
#include "llvm/Support/TargetSelect.h"
#include "llvm/Support/raw_ostream.h"

using namespace clang;
using namespace clang::tooling;

static llvm::cl::OptionCategory category("i86clang");
static llvm::cl::opt<std::string> Model("model", llvm::cl::init("medium"), llvm::cl::cat(category),
                                        llvm::cl::desc("small, medium, compact, large or huge"));
// A huge object is over 64K, so its size (and sizeof) needs 32 bits; Borland's size_t stays 16.
static llvm::cl::opt<std::string> Dump("dump-annotated", llvm::cl::desc("write the source with its pointers marked here"), llvm::cl::cat(category));
static llvm::cl::opt<bool> WideSize("wide-size", llvm::cl::desc("32-bit size_t"), llvm::cl::cat(category));

static bool codeFar() { return Model == "medium" || Model == "large" || Model == "huge"; }
static bool dataFar() { return Model == "compact" || Model == "large" || Model == "huge"; }

// llrm's datalayout (index 16 on the far space, 32 on the huge one), which clang reads for every pointer's width.
static const char *kLlrmLayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-n8:16:32-ni:2";

// TargetOpts is private and set by CreateTargetInfo, which a target built inside clang would use.
template <class Tag, typename Tag::type M> struct Rob { friend typename Tag::type get(Tag) { return M; } };
struct TargetOptsTag { typedef TargetOptions *TargetInfo::*type; friend type get(TargetOptsTag); };
template struct Rob<TargetOptsTag, &TargetInfo::TargetOpts>;

// `address_space(N)` is the target's space N; the two pointer-size qualifiers reach the same spaces.
static LangASMap spaceMap;

class I86Target : public TargetInfo {
public:
  I86Target(const llvm::Triple &t, const TargetOptions &o) : TargetInfo(t) {
    this->*get(TargetOptsTag()) = const_cast<TargetOptions *>(&o);
    // Microsoft's `__ptr64` and `__uptr` qualify a pointer only where a default one is 32 bits wide (clang's rule for them).
    PointerWidth = 32;
    for (unsigned &one : spaceMap) one = 0;
    spaceMap[(unsigned)LangAS::ptr64] = 1;
    AddrSpaceMap = &spaceMap;
    PointerAlign = 16; IntWidth = IntAlign = 16; LongWidth = 32; LongAlign = 16;
    LongLongWidth = 64; LongLongAlign = 16; SuitableAlign = 16;
    SizeType = WideSize ? UnsignedLong : UnsignedInt;
    PtrDiffType = WideSize || dataFar() ? SignedLong : SignedInt;
    IntPtrType = SignedInt; WCharType = SignedInt; Char16Type = UnsignedInt; Char32Type = UnsignedLong;
    resetDataLayout("e-m:e-p:16:16-i32:16-i64:16-f32:16-f64:16-a:8-n8:16-S16");
  }
  // Borland's pascal is clang's stdcall here (llrm's convention 1000).
  CallingConvCheckResult checkCallingConvention(CallingConv cc) const override {
    return cc == CC_C || cc == CC_X86StdCall ? CCCR_OK : CCCR_Warning;
  }
  // `address_space(N)` is the target's space N: 0 is near, the rest are segmented.
  uint64_t getPointerWidthV(LangAS as) const override {
    if (as == LangAS::ptr32_uptr) return 16;
    return isTargetAddressSpace(as) && toTargetAddressSpace(as) == 0 ? 16 : 32;
  }
  uint64_t getPointerAlignV(LangAS) const override { return 16; }
  // Clang computes a pointer difference in an integer this wide (`ptrtoint`, `sub`): ptrdiff_t's, so that far pointers, which
  // Borland and Watcom subtract by their offsets alone, give the offsets' difference (the low word of the packed pointers).
  uint64_t getMaxPointerWidth() const override { return dataFar() || WideSize ? 32 : 16; }
  // Borland converts between near and far pointers without a cast (a warning): near to far adds DS, far to near keeps the offset.
  bool isAddressSpaceSupersetOf(LangAS, LangAS) const override { return true; }
  void getTargetDefines(const LangOptions &, MacroBuilder &b) const override { b.defineMacro("__i86__"); }
  llvm::SmallVector<Builtin::InfosShard> getTargetBuiltins() const override { return {}; }
  BuiltinVaListKind getBuiltinVaListKind() const override { return VoidPtrBuiltinVaList; }
  bool validateAsmConstraint(const char *&, TargetInfo::ConstraintInfo &) const override { return false; }
  std::string_view getClobbers() const override { return ""; }
  ArrayRef<const char *> getGCCRegNames() const override { return {}; }
  ArrayRef<GCCRegAlias> getGCCRegAliases() const override { return {}; }
};

struct IrConsumer : ASTConsumer {
  CompilerInstance &ci;
  llvm::LLVMContext context;
  std::unique_ptr<CodeGenerator> generator;
  CodeGenOptions options;
  explicit IrConsumer(CompilerInstance &c) : ci(c), options(c.getCodeGenOpts()) {
    options.DisableO0ImplyOptNone = true;
    options.OptimizationLevel = 0;
    generator.reset(CreateLLVMCodeGen(c.getDiagnostics(), "m", &c.getVirtualFileSystem(), c.getHeaderSearchOpts(),
                                      c.getPreprocessorOpts(), options, context));
  }
  void Initialize(ASTContext &ctx) override {
    generator->Initialize(ctx);
    // P1 stays on the module while clang creates the functions, then goes (llrm-llc wants its own layout, program space 0).
    generator->GetModule()->setDataLayout(std::string(kLlrmLayout) + (codeFar() ? "-P1" : ""));
    generator->GetModule()->setTargetTriple(llvm::Triple());
  }
  bool HandleTopLevelDecl(DeclGroupRef d) override { return generator->HandleTopLevelDecl(d); }
  void HandleInlineFunctionDefinition(FunctionDecl *d) override { generator->HandleInlineFunctionDefinition(d); }
  void HandleInterestingDecl(DeclGroupRef d) override { generator->HandleInterestingDecl(d); }
  void HandleTagDeclDefinition(TagDecl *d) override { generator->HandleTagDeclDefinition(d); }
  void HandleTagDeclRequiredDefinition(const TagDecl *d) override { generator->HandleTagDeclRequiredDefinition(d); }
  void HandleCXXImplicitFunctionInstantiation(FunctionDecl *d) override { generator->HandleCXXImplicitFunctionInstantiation(d); }
  void HandleTopLevelDeclInObjCContainer(DeclGroupRef d) override { generator->HandleTopLevelDeclInObjCContainer(d); }
  void HandleImplicitImportDecl(ImportDecl *d) override { generator->HandleImplicitImportDecl(d); }
  void CompleteTentativeDefinition(VarDecl *d) override { generator->CompleteTentativeDefinition(d); }
  void CompleteExternalDeclaration(DeclaratorDecl *d) override { generator->CompleteExternalDeclaration(d); }
  void AssignInheritanceModel(CXXRecordDecl *d) override { generator->AssignInheritanceModel(d); }
  void HandleVTable(CXXRecordDecl *d) override { generator->HandleVTable(d); }
  void HandleTranslationUnit(ASTContext &ctx) override {
    generator->HandleTranslationUnit(ctx);
    if (ci.getDiagnostics().hasErrorOccurred()) return;
    llvm::Module &m = *generator->GetModule();
    // A huge object lives in a far segment (space 1); only its pointers are huge (space 3), as llrm-c lays it out.
    std::vector<llvm::GlobalVariable *> huge;
    for (llvm::GlobalVariable &g : m.globals()) if (g.getAddressSpace() == 3) huge.push_back(&g);
    for (llvm::GlobalVariable *g : huge) {
      auto *far = new llvm::GlobalVariable(m, g->getValueType(), g->isConstant(), g->getLinkage(),
                                           g->hasInitializer() ? g->getInitializer() : nullptr, "", nullptr,
                                           g->getThreadLocalMode(), 1, g->isExternallyInitialized());
      far->setAlignment(g->getAlign());
      far->setVisibility(g->getVisibility());
      far->takeName(g);
      g->replaceAllUsesWith(llvm::ConstantExpr::getAddrSpaceCast(far, g->getType()));
      g->eraseFromParent();
    }
    // A huge pointer's packed bits are not its address: llrm subtracts two of them by an intrinsic of its own.
    for (llvm::Function &f : m) {
      std::vector<llvm::BinaryOperator *> subs;
      for (llvm::BasicBlock &b : f)
        for (llvm::Instruction &i : b)
          if (auto *s = llvm::dyn_cast<llvm::BinaryOperator>(&i); s && s->getOpcode() == llvm::Instruction::Sub) subs.push_back(s);
      for (llvm::BinaryOperator *s : subs) {
        auto *a = llvm::dyn_cast<llvm::PtrToIntInst>(s->getOperand(0)), *b = llvm::dyn_cast<llvm::PtrToIntInst>(s->getOperand(1));
        if (!a || !b || a->getPointerAddressSpace() != 3 || b->getPointerAddressSpace() != 3) continue;
        llvm::Type *i32 = llvm::Type::getInt32Ty(context);
        // The intrinsic is declared in space 0: a name llrm-llc lowers, not a function of the program.
        llvm::Function *diff = m.getFunction("llrm.ia16.ptrdiff.i32.p3");
        if (!diff) {
          diff = llvm::Function::Create(llvm::FunctionType::get(i32, {a->getPointerOperand()->getType(), b->getPointerOperand()->getType()}, false),
                                        llvm::GlobalValue::ExternalLinkage, 0, "llrm.ia16.ptrdiff.i32.p3", &m);
          diff->addFnAttr(llvm::Attribute::NoUnwind);
        }
        llvm::IRBuilder<> at(s);
        llvm::Value *d = at.CreateCall(diff, {a->getPointerOperand(), b->getPointerOperand()});
        s->replaceAllUsesWith(at.CreateTrunc(at.CreateSExtOrTrunc(d, s->getType()), s->getType()));
        s->eraseFromParent();
      }
    }
    // Borland's pascal (clang's stdcall) is llrm's calling convention 1000. Names are llrm-llc's to spell, by convention.
    constexpr unsigned kPascal = 1000;
    for (llvm::Function &f : m) {
      if (f.getCallingConv() == llvm::CallingConv::X86_StdCall) f.setCallingConv(kPascal);
      for (llvm::BasicBlock &b : f)
        for (llvm::Instruction &i : b)
          if (auto *c = llvm::dyn_cast<llvm::CallBase>(&i); c && c->getCallingConv() == llvm::CallingConv::X86_StdCall)
            c->setCallingConv(kPascal);
    }
    m.setDataLayout(kLlrmLayout);
    m.print(llvm::outs(), nullptr);
  }
};

struct Emit : ASTFrontendAction {
  bool BeginSourceFileAction(CompilerInstance &ci) override {
    ci.getTarget().Retain();
    ci.setTarget(new I86Target(ci.getTarget().getTriple(), ci.getTargetOpts()));
    return true;
  }
  std::unique_ptr<ASTConsumer> CreateASTConsumer(CompilerInstance &ci, StringRef) override {
    return std::make_unique<IrConsumer>(ci);
  }
};

// ---- the first parse: every data pointer the source leaves unmarked gets the model's default distance ----------------------------
// The space is written just before the `*`: `char *` is `char AS *` (the pointee in the space), `char **` is `char AS * AS *`. A
// pointer to a function stays as it is (it is as wide as code is far), as does one that already names a space (`far`, `near`,
// `huge`). The space then sits in the declaration specifiers, where it would qualify every declarator of a list, so a list with
// a pointer in it is split into declarations of one declarator each (`char *a, c;` is `char AS *a; char c;`).
struct Edit { unsigned at, drop; std::string text; };

// The type a declaration list shares: what is left of a declarator's type without its pointers, arrays and parameters.
static TypeLoc specifier(TypeLoc tl) {
  for (;;) {
    if (auto p = tl.getAs<PointerTypeLoc>()) tl = p.getPointeeLoc();
    else if (auto a = tl.getAs<ArrayTypeLoc>()) tl = a.getElementLoc();
    else if (auto f = tl.getAs<FunctionTypeLoc>()) tl = f.getReturnLoc();
    else if (auto p = tl.getAs<ParenTypeLoc>()) tl = p.getInnerLoc();
    else if (auto q = tl.getAs<QualifiedTypeLoc>()) tl = q.getUnqualifiedLoc();
    else if (auto t = tl.getAs<AttributedTypeLoc>()) tl = t.getModifiedLoc();
    else return tl;
  }
}

struct Stars : RecursiveASTVisitor<Stars> {
  ASTContext &ctx;
  SourceManager &sm;
  std::set<unsigned> said, stars, codes;  // the `*` of each data pointer, and of each function pointer, with no distance of its own
  std::map<unsigned, std::vector<DeclaratorDecl *>> lists;  // by the start of the declaration they share
  explicit Stars(ASTContext &c) : ctx(c), sm(c.getSourceManager()) {}
  bool local(SourceLocation l) const { return l.isValid() && !l.isMacroID() && sm.isInMainFile(l); }
  // `__ptr64` and `__uptr` are sugar until code generation: a pointer under one of them already says its distance.
  bool VisitAttributedTypeLoc(AttributedTypeLoc tl) {
    switch (tl.getAttrKind()) {
    case attr::Ptr32: case attr::Ptr64: case attr::SPtr: case attr::UPtr:
      if (auto pointer = tl.getModifiedLoc().getAs<PointerTypeLoc>(); pointer && local(pointer.getStarLoc()))
        said.insert(sm.getFileOffset(pointer.getStarLoc()));
      break;
    default: break;
    }
    return true;
  }
  bool VisitPointerTypeLoc(PointerTypeLoc tl) {
    QualType pointee = tl.getPointeeLoc().getType();
    if (pointee.hasAddressSpace() || pointee->isArrayType() || !local(tl.getStarLoc()) || said.count(sm.getFileOffset(tl.getStarLoc()))) return true;
    (pointee->isFunctionType() ? codes : stars).insert(sm.getFileOffset(tl.getStarLoc()));
    return true;
  }
  void list(DeclaratorDecl *d) {
    if (!local(d->getOuterLocStart()) || d->isImplicit()) return;
    lists[sm.getFileOffset(d->getOuterLocStart())].push_back(d);
  }
  bool VisitVarDecl(VarDecl *d) { if (!isa<ParmVarDecl>(d)) list(d); return true; }
  bool VisitFieldDecl(FieldDecl *d) { list(d); return true; }

  std::string text(SourceLocation from, SourceLocation toTokenEnd) const {
    return std::string(sm.getBufferData(sm.getMainFileID()).substr(sm.getFileOffset(from), sm.getFileOffset(toTokenEnd) - sm.getFileOffset(from)));
  }
  bool marked(DeclaratorDecl *d) const {
    if (!d->getTypeSourceInfo() || !local(d->getLocation())) return false;
    unsigned from = sm.getFileOffset(d->getTypeSpecStartLoc()), to = sm.getFileOffset(d->getLocation());
    auto it = stars.lower_bound(from);
    return it != stars.end() && *it < to;
  }
  std::vector<Edit> edits() const {
    std::vector<Edit> out;
    std::string mark = std::string("__attribute__((address_space(") + (dataFar() ? "1" : "0") + "))) ";
    for (unsigned at : stars) out.push_back({at, 0, mark});
    // After the star, where a pointer qualifier goes: `int (* __ptr64 f)(int)`.
    for (unsigned at : codes) out.push_back({at + 1, 0, codeFar() ? " __ptr64 " : " __uptr "});
    for (auto &[start, members] : lists) {
      if (members.size() < 2 || !std::any_of(members.begin(), members.end(), [&](DeclaratorDecl *d) { return marked(d); })) continue;
      DeclaratorDecl *first = members[0];
      if (!first->getTypeSourceInfo()) continue;
      SourceLocation end = specifier(first->getTypeSourceInfo()->getTypeLoc()).getEndLoc();
      if (end.isInvalid() || first->getTypeSpecStartLoc().isInvalid()) continue;
      // A type that is itself defined here (`struct { .. } a, *b;`) cannot be written again.
      if (auto *tag = first->getType()->getAsTagDecl(); tag && tag->isThisDeclarationADefinition() && local(tag->getBeginLoc()) &&
          sm.getFileOffset(tag->getBeginLoc()) >= sm.getFileOffset(first->getOuterLocStart()))
        continue;
      std::string base = text(first->getOuterLocStart(), Lexer::getLocForEndOfToken(end, 0, sm, ctx.getLangOpts()));
      for (size_t i = 1; i < members.size(); ++i) {
        // The separator is the last comma before this declarator's name: what lies between holds none (`*`, qualifiers).
        llvm::StringRef buffer = sm.getBufferData(sm.getMainFileID());
        unsigned name = sm.getFileOffset(members[i]->getLocation()), floor = sm.getFileOffset(members[i - 1]->getLocation());
        size_t comma = buffer.substr(0, name).rfind(',');
        if (comma == llvm::StringRef::npos || comma < floor) continue;
        out.push_back({(unsigned)comma, 1, "; " + base});
      }
    }
    std::sort(out.begin(), out.end(), [](const Edit &a, const Edit &b) { return a.at > b.at; });
    return out;
  }
};
struct Annotate : ASTFrontendAction {
  std::vector<Edit> &edits;
  explicit Annotate(std::vector<Edit> &e) : edits(e) {}
  bool BeginSourceFileAction(CompilerInstance &ci) override {
    ci.getTarget().Retain();
    ci.setTarget(new I86Target(ci.getTarget().getTriple(), ci.getTargetOpts()));
    return true;
  }
  std::unique_ptr<ASTConsumer> CreateASTConsumer(CompilerInstance &, StringRef) override {
    struct C : ASTConsumer {
      std::vector<Edit> &edits;
      explicit C(std::vector<Edit> &e) : edits(e) {}
      void HandleTranslationUnit(ASTContext &ctx) override {
        Stars stars(ctx);
        stars.TraverseDecl(ctx.getTranslationUnitDecl());
        edits = stars.edits();
      }
    };
    return std::make_unique<C>(edits);
  }
};

static std::string annotated(const std::string &code, const std::vector<std::string> &flags, const std::string &file) {
  std::vector<Edit> edits;
  std::vector<std::string> args = flags;
  args.push_back("-w");
  if (!runToolOnCodeWithArgs(std::make_unique<Annotate>(edits), code, args, file)) return code;
  std::string text = code;
  for (const Edit &e : edits) text.replace(e.at, e.drop, e.text);
  return text;
}

int main(int argc, const char **argv) {
  llvm::InitializeAllTargets(); llvm::InitializeAllTargetMCs(); llvm::InitializeAllAsmPrinters();
  auto parser = CommonOptionsParser::create(argc, argv, category);
  if (!parser) { llvm::errs() << parser.takeError(); return 1; }
  if (!llvm::is_contained(std::vector<std::string>{"small", "medium", "compact", "large", "huge"}, Model.getValue())) {
    llvm::errs() << "i86clang: -model is small, medium, compact, large or huge\n";
    return 1;
  }
  auto &sources = parser->getSourcePathList();
  if (sources.size() != 1) { llvm::errs() << "i86clang: one source file\n"; return 1; }
  auto commands = parser->getCompilations().getCompileCommands(sources[0]);
  if (commands.empty()) { llvm::errs() << "i86clang: no compile command\n"; return 1; }
  // The command line is `clang <flags> <file>`; the code is run on the source with the stars marked.
  std::vector<std::string> flags(commands[0].CommandLine.begin() + 1, commands[0].CommandLine.end() - 1);
  flags.push_back("-fms-extensions");  // the spelling of `__ptr64` and `__uptr`
  auto buffer = llvm::MemoryBuffer::getFile(sources[0]);
  if (!buffer) { llvm::errs() << "i86clang: cannot read " << sources[0] << "\n"; return 1; }
  std::string code = annotated((*buffer)->getBuffer().str(), flags, sources[0]);
  if (Dump.size()) { std::error_code ec; llvm::raw_fd_ostream(Dump, ec) << code; }
  return runToolOnCodeWithArgs(std::make_unique<Emit>(), code, flags, sources[0]) ? 0 : 1;
}
