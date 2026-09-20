//! Ordered, target-independent function pass execution.
//!
//! This module deliberately owns pass scheduling and observability only.  It
//! does not cache analyses or prescribe how a pass edits IR.  Analyses remain
//! values owned by the analysis subsystem; a future cache can use the
//! invalidation information here without making transforms depend on it.

use std::fmt;

use crate::ir::{Function, FunctionId, Module};
use crate::support::diagnostic::Diagnostic;

/// The analyses a pass guarantees remain valid after it runs.
///
/// The initial representation is intentionally coarse.  It makes no claim
/// about concrete analysis types, while still giving a future analysis cache
/// a sound all-or-nothing invalidation contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PreservedAnalyses {
    /// Every analysis result remains valid.
    All,
    /// No analysis result is guaranteed to remain valid.
    None,
}

impl PreservedAnalyses {
    /// Returns the complementary invalidation description.
    pub const fn invalidated(self) -> AnalysisInvalidation {
        match self {
            Self::All => AnalysisInvalidation::None,
            Self::None => AnalysisInvalidation::All,
        }
    }
}

/// The analysis results a pass invalidates.
///
/// This is a description, not an analysis cache.  The analysis subsystem owns
/// any cached results and decides how to act on it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AnalysisInvalidation {
    /// No analysis result is invalidated.
    None,
    /// Every analysis result must be recomputed before reuse.
    All,
}

/// The result reported by a successful function pass.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PassOutcome {
    changed: bool,
    preserved_analyses: PreservedAnalyses,
}

impl PassOutcome {
    /// Creates an unchanged outcome.
    pub const fn unchanged() -> Self {
        Self {
            changed: false,
            preserved_analyses: PreservedAnalyses::All,
        }
    }

    /// Creates a changed outcome with the stated analysis guarantee.
    pub const fn changed(preserved_analyses: PreservedAnalyses) -> Self {
        Self {
            changed: true,
            preserved_analyses,
        }
    }

    /// Whether the pass changed the function.
    pub const fn changed_ir(self) -> bool {
        self.changed
    }

    /// The analyses still valid after the pass.
    pub const fn preserved_analyses(self) -> PreservedAnalyses {
        self.preserved_analyses
    }

    /// The analyses invalidated by the pass.
    pub const fn invalidated_analyses(self) -> AnalysisInvalidation {
        self.preserved_analyses.invalidated()
    }
}

/// A failure reported by a pass before the manager adds execution context.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PassFailure {
    message: String,
}

impl PassFailure {
    /// Creates a pass failure with a diagnostic message.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    /// The pass-provided failure message.
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl From<String> for PassFailure {
    fn from(message: String) -> Self {
        Self::new(message)
    }
}

impl From<&str> for PassFailure {
    fn from(message: &str) -> Self {
        Self::new(message)
    }
}

/// Identifies the function affected by a pass execution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FunctionIdentity {
    pub id: FunctionId,
    pub name: String,
}

impl FunctionIdentity {
    fn from_function(function: &Function) -> Self {
        Self {
            id: function.id,
            name: function.name.clone(),
        }
    }
}

/// A failure executing or verifying a pass.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PassError {
    /// A pass explicitly refused or failed to process one function.
    PassFailed {
        pass_name: &'static str,
        function: FunctionIdentity,
        failure: PassFailure,
    },
    /// A changed function made the containing module fail structural
    /// verification.
    VerificationFailed {
        pass_name: &'static str,
        function: FunctionIdentity,
        diagnostics: Vec<Diagnostic>,
    },
}

impl PassError {
    /// The pass whose execution or verification failed.
    pub const fn pass_name(&self) -> &'static str {
        match self {
            Self::PassFailed { pass_name, .. } | Self::VerificationFailed { pass_name, .. } => {
                pass_name
            }
        }
    }

    /// The function affected by the failure.
    pub fn function(&self) -> &FunctionIdentity {
        match self {
            Self::PassFailed { function, .. } | Self::VerificationFailed { function, .. } => {
                function
            }
        }
    }
}

impl fmt::Display for PassError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PassFailed {
                pass_name,
                function,
                failure,
            } => write!(
                formatter,
                "pass {pass_name} failed for function {} ({}): {}",
                function.name, function.id, failure.message
            ),
            Self::VerificationFailed {
                pass_name,
                function,
                diagnostics,
            } => write!(
                formatter,
                "pass {pass_name} produced invalid IR for function {} ({}) with {} diagnostic(s)",
                function.name,
                function.id,
                diagnostics.len()
            ),
        }
    }
}

impl std::error::Error for PassError {}

/// A target-independent transformation of one portable IR function.
pub trait FunctionPass {
    /// A stable, human-readable pass name for diagnostics and instrumentation.
    fn name(&self) -> &'static str;

    /// Runs this pass over one function.
    fn run(&mut self, function: &mut Function) -> Result<PassOutcome, PassFailure>;
}

/// Receives immutable before/after views of pass execution.
///
/// Implementations decide whether to record, render, or ignore these events.
/// The manager itself never performs output, which keeps it usable by the
/// driver and textual tools alike.
pub trait PassInstrumentation {
    /// Called immediately before a pass receives mutable access to a function.
    fn before_pass(&mut self, _pass_name: &str, _function: &Function) {}

    /// Called after a pass succeeds, and after optional verification succeeds.
    fn after_pass(&mut self, _pass_name: &str, _function: &Function, _outcome: PassOutcome) {}

    /// Called when a pass or its post-pass verification fails.
    fn pass_failed(&mut self, _pass_name: &str, _function: &Function, _error: &PassError) {}
}

/// One successful pass execution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PassExecution {
    pub pass_name: &'static str,
    pub function: FunctionIdentity,
    pub outcome: PassOutcome,
}

/// The ordered successful executions of a pass pipeline.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FunctionPassReport {
    executions: Vec<PassExecution>,
}

impl FunctionPassReport {
    /// The successful executions in deterministic function/pipeline order.
    pub fn executions(&self) -> &[PassExecution] {
        &self.executions
    }
}

/// An ordered manager for target-independent function passes.
#[derive(Default)]
pub struct FunctionPassManager {
    passes: Vec<Box<dyn FunctionPass>>,
    instrumentation: Vec<Box<dyn PassInstrumentation>>,
    verify_each: bool,
}

impl FunctionPassManager {
    /// Creates an empty pass manager.
    pub fn new() -> Self {
        Self::default()
    }

    /// Configures structural verification after every changed pass.
    pub fn set_verify_each(&mut self, enabled: bool) {
        self.verify_each = enabled;
    }

    /// Whether structural verification after changed passes is enabled.
    pub const fn verify_each(&self) -> bool {
        self.verify_each
    }

    /// Appends a pass to this manager's deterministic pipeline.
    pub fn add_pass(&mut self, pass: impl FunctionPass + 'static) {
        self.passes.push(Box::new(pass));
    }

    /// Appends an observer for pass lifecycle events.
    pub fn add_instrumentation(&mut self, instrumentation: impl PassInstrumentation + 'static) {
        self.instrumentation.push(Box::new(instrumentation));
    }

    /// Runs every pass over every function in module order.
    ///
    /// When `verify_each` is enabled, only a pass that reports a change pays
    /// for a module verification.  A verifier failure is attributed to the
    /// pass and function that immediately preceded it.
    pub fn run(&mut self, module: &mut Module) -> Result<FunctionPassReport, PassError> {
        let mut report = FunctionPassReport::default();

        for function_index in 0..module.functions.len() {
            for pass_index in 0..self.passes.len() {
                let pass_name = self.passes[pass_index].name();
                self.notify_before(pass_name, &module.functions[function_index]);
                let original = module.functions[function_index].clone();
                let identity = FunctionIdentity::from_function(&original);

                let outcome = match self.passes[pass_index]
                    .run(&mut module.functions[function_index])
                {
                    Ok(outcome) => outcome,
                    Err(failure) => {
                        let error = PassError::PassFailed {
                            pass_name,
                            function: identity,
                            failure,
                        };
                        module.functions[function_index] = original;
                        self.notify_failure(pass_name, &module.functions[function_index], &error);
                        return Err(error);
                    }
                };

                if self.verify_each && outcome.changed_ir() {
                    if let Err(diagnostics) = module.verify() {
                        let error = PassError::VerificationFailed {
                            pass_name,
                            function: identity,
                            diagnostics,
                        };
                        module.functions[function_index] = original;
                        self.notify_failure(pass_name, &module.functions[function_index], &error);
                        return Err(error);
                    }
                }

                self.notify_after(pass_name, &module.functions[function_index], outcome);
                report.executions.push(PassExecution {
                    pass_name,
                    function: FunctionIdentity::from_function(&module.functions[function_index]),
                    outcome,
                });
            }
        }

        Ok(report)
    }

    fn notify_before(&mut self, pass_name: &str, function: &Function) {
        for instrumentation in &mut self.instrumentation {
            instrumentation.before_pass(pass_name, function);
        }
    }

    fn notify_after(&mut self, pass_name: &str, function: &Function, outcome: PassOutcome) {
        for instrumentation in &mut self.instrumentation {
            instrumentation.after_pass(pass_name, function, outcome);
        }
    }

    fn notify_failure(&mut self, pass_name: &str, function: &Function, error: &PassError) {
        for instrumentation in &mut self.instrumentation {
            instrumentation.pass_failed(pass_name, function, error);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;
    use crate::ir::{
        Block, BlockId, CallingConvention, Function, FunctionId, Linkage, Signature, Terminator,
        Type, TypeId, TypeKind,
    };

    struct RenamePass {
        name: &'static str,
        suffix: &'static str,
        changed: bool,
    }

    impl FunctionPass for RenamePass {
        fn name(&self) -> &'static str {
            self.name
        }

        fn run(&mut self, function: &mut Function) -> Result<PassOutcome, PassFailure> {
            if self.changed {
                function.name.push_str(self.suffix);
                Ok(PassOutcome::changed(PreservedAnalyses::None))
            } else {
                Ok(PassOutcome::unchanged())
            }
        }
    }

    struct BreakFunctionPass;

    impl FunctionPass for BreakFunctionPass {
        fn name(&self) -> &'static str {
            "break-function"
        }

        fn run(&mut self, function: &mut Function) -> Result<PassOutcome, PassFailure> {
            function.blocks.clear();
            Ok(PassOutcome::changed(PreservedAnalyses::None))
        }
    }

    struct Events {
        entries: Arc<Mutex<Vec<String>>>,
    }

    impl Events {
        fn new(entries: Arc<Mutex<Vec<String>>>) -> Self {
            Self { entries }
        }
    }

    impl PassInstrumentation for Events {
        fn before_pass(&mut self, pass_name: &str, function: &Function) {
            self.entries
                .lock()
                .expect("test event lock should not be poisoned")
                .push(format!("before:{pass_name}:{}", function.name));
        }

        fn after_pass(&mut self, pass_name: &str, function: &Function, _: PassOutcome) {
            self.entries
                .lock()
                .expect("test event lock should not be poisoned")
                .push(format!("after:{pass_name}:{}", function.name));
        }

        fn pass_failed(&mut self, pass_name: &str, function: &Function, _: &PassError) {
            self.entries
                .lock()
                .expect("test event lock should not be poisoned")
                .push(format!("failed:{pass_name}:{}", function.name));
        }
    }

    fn module() -> Module {
        Module {
            name: "test".into(),
            types: vec![Type {
                id: TypeId::new(0),
                kind: TypeKind::Void,
            }],
            globals: Vec::new(),
            functions: vec![Function {
                id: FunctionId::new(0),
                name: "entry".into(),
                signature: Signature {
                    result: TypeId::new(0),
                    parameters: Vec::new(),
                    variadic: false,
                    calling_convention: CallingConvention::C,
                },
                linkage: Linkage::Internal,
                attributes: Vec::new(),
                parameters: Vec::new(),
                blocks: vec![Block {
                    id: BlockId::new(0),
                    instructions: Vec::new(),
                    terminator: Terminator::Return(None),
                }],
            }],
        }
    }

    #[test]
    fn runs_passes_in_order_and_records_changed_outcomes() {
        let mut manager = FunctionPassManager::new();
        manager.add_pass(RenamePass {
            name: "first",
            suffix: "-first",
            changed: true,
        });
        manager.add_pass(RenamePass {
            name: "second",
            suffix: "-second",
            changed: false,
        });
        let mut module = module();

        let report = manager.run(&mut module).expect("passes should succeed");

        assert_eq!(module.functions[0].name, "entry-first");
        assert_eq!(report.executions().len(), 2);
        assert_eq!(report.executions()[0].pass_name, "first");
        assert!(report.executions()[0].outcome.changed_ir());
        assert_eq!(
            report.executions()[0].outcome.invalidated_analyses(),
            AnalysisInvalidation::All
        );
        assert_eq!(report.executions()[1].pass_name, "second");
        assert!(!report.executions()[1].outcome.changed_ir());
        assert_eq!(
            report.executions()[1].outcome.preserved_analyses(),
            PreservedAnalyses::All
        );
    }

    #[test]
    fn verification_failure_is_attributed_to_the_changing_pass() {
        let mut manager = FunctionPassManager::new();
        manager.set_verify_each(true);
        manager.add_pass(BreakFunctionPass);
        let mut module = module();

        let error = manager
            .run(&mut module)
            .expect_err("verification should fail");

        match error {
            PassError::VerificationFailed {
                pass_name,
                function,
                diagnostics,
            } => {
                assert_eq!(pass_name, "break-function");
                assert_eq!(function.id, FunctionId::new(0));
                assert_eq!(function.name, "entry");
                assert!(
                    diagnostics.iter().any(|diagnostic| diagnostic
                        .message
                        .contains("declaration without blocks"))
                );
            }
            other => panic!("expected verification failure, got {other:?}"),
        }
        assert_eq!(module.functions[0].blocks.len(), 1);
        assert!(matches!(
            module.functions[0].blocks[0].terminator,
            Terminator::Return(None)
        ));
    }

    #[test]
    fn instrumentation_observes_before_and_after_in_pipeline_order() {
        let mut manager = FunctionPassManager::new();
        manager.add_pass(RenamePass {
            name: "first",
            suffix: "-first",
            changed: true,
        });
        manager.add_pass(RenamePass {
            name: "second",
            suffix: "-second",
            changed: true,
        });
        let entries = Arc::new(Mutex::new(Vec::new()));
        manager.add_instrumentation(Events::new(Arc::clone(&entries)));
        let mut module = module();

        manager.run(&mut module).expect("passes should succeed");

        let observed = entries
            .lock()
            .expect("test event lock should not be poisoned")
            .clone();
        assert_eq!(
            observed,
            vec![
                "before:first:entry",
                "after:first:entry-first",
                "before:second:entry-first",
                "after:second:entry-first-second",
            ]
        );
    }
}
