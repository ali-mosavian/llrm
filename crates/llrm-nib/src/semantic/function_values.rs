//! Function values (section 3). A value of `fn(A) -> R` is a small integer
//! naming one of the functions converted to that type, and a call through it
//! is a call of the type's dispatcher, which calls the function it names.
//! The program is compiled whole, so a type's functions are all known once
//! everything that converts to it is compiled: a function value reduces to a
//! value and direct calls.

use super::*;
use crate::syntax::{LambdaParameter, MatchArm, Parameter, Pattern};

#[derive(Clone, Debug)]
pub(super) struct FunctionType {
    /// The type as a header spells it, which its dispatcher takes and returns.
    parameters: Vec<ParameterType>,
    result: TypeAnnotation,
    /// The functions converted to this type; a value is an index.
    members: Vec<String>,
    /// Its dispatcher, once a call through a value needs it, and whether
    /// its body was built.
    dispatcher: Option<(String, bool)>,
}

impl TypeRegistry {
    /// `fn(A) -> R` as written: the type of a function declared so.
    pub(super) fn function_type_spelled(&mut self, args: &[TypeAnnotation], span: Span) -> Result<TypeName, Diagnostic> {
        let (result, parameters) = args.split_last().expect("a function type has a result");
        let parameters = parameters
            .iter()
            .enumerate()
            .map(|(index, one)| Parameter {
                name: format!("${index}"),
                type_: match one {
                    TypeAnnotation::Value(TypeSpec::Applied { name, args }) if name.starts_with('&') => ParameterType::Borrowed {
                        mutable: name == "&mut",
                        target: args[0].clone(),
                    },
                    other => ParameterType::Owned(other.clone()),
                },
                default: None,
                span,
            })
            .collect();
        let declared = Function { name: String::new(), generics: Vec::new(), parameters, result: result.clone(), body: Vec::new(), span };
        let signature = signature(self, &declared, 0)?;
        Ok(self.function_type(&signature))
    }

    /// The type of a function with `signature`, registered on first use.
    pub(super) fn function_type(&mut self, signature: &Signature) -> TypeName {
        if let Some(found) = self.function_type_of(signature) {
            return found;
        }
        let (parameters, result) = self.signature_syntax(signature);
        let type_id = self.types.len() as u32 + 1;
        let name = spelled(&parameters, &result).text();
        self.types.push(plain_type(type_id, &name, "integer", 2, Some(false), "none"));
        self.function_types.insert(type_id, FunctionType { parameters, result, members: Vec::new(), dispatcher: None });
        TypeName::Function { type_id }
    }

    /// The type of a function with `signature`, if one was registered.
    pub(super) fn function_type_of(&self, signature: &Signature) -> Option<TypeName> {
        let (parameters, result) = self.signature_syntax(signature);
        let name = spelled(&parameters, &result).text();
        self.function_types
            .keys()
            .find(|type_id| self.types[(**type_id - 1) as usize].name == name)
            .map(|&type_id| TypeName::Function { type_id })
    }

    /// The value naming `function` among `type_name`'s functions.
    fn member(&mut self, type_name: TypeName, function: &str) -> i64 {
        let TypeName::Function { type_id } = type_name else {
            unreachable!("a function type")
        };
        let members = &mut self.function_types.get_mut(&type_id).expect("registered").members;
        let index = members.iter().position(|one| one == function).unwrap_or_else(|| {
            members.push(function.into());
            members.len() - 1
        });
        index as i64
    }

    /// How a header spells `signature`'s parameters and result.
    fn signature_syntax(&self, signature: &Signature) -> (Vec<ParameterType>, TypeAnnotation) {
        let annotation = |binding: BindingType| match binding {
            BindingType::Scalar(type_name) => TypeAnnotation::Value(TypeSpec::Primitive(type_name)),
            BindingType::Struct(id) => TypeAnnotation::Value(self.spec_of(ElementType::Struct(id))),
            BindingType::Slice { element, rank } => TypeAnnotation::Slice { element: self.spec_of(element), rank },
            BindingType::Array { element, shape } => TypeAnnotation::Array { element: self.spec_of(element), dims: shape.dims().to_vec() },
        };
        let parameters = signature
            .parameters
            .iter()
            .map(|one| match *one {
                SignatureParameter::Scalar(type_name) | SignatureParameter::Adapter { pointer: type_name, .. } => {
                    ParameterType::Owned(annotation(BindingType::Scalar(type_name)))
                }
                SignatureParameter::Owned { struct_id, .. } => ParameterType::Owned(annotation(BindingType::Struct(struct_id))),
                SignatureParameter::Borrowed { mutable, target, .. } => ParameterType::Borrowed { mutable, target: annotation(target) },
            })
            .collect();
        let result = match (signature.view, signature.slot) {
            (Some((element, rank)), _) => annotation(BindingType::Slice { element, rank }),
            (None, Some(struct_id)) => annotation(self.aggregate_binding(struct_id)),
            (None, None) => annotation(BindingType::Scalar(signature.result)),
        };
        (parameters, result)
    }

    /// Each dispatcher a call needs whose body is not built yet, built: a
    /// `match` of the value over the type's functions, each called with the
    /// dispatcher's arguments.
    pub(super) fn dispatcher_bodies(&mut self, span: Span) -> Vec<Function> {
        let mut built = Vec::new();
        for (&type_id, kind) in &mut self.function_types {
            let Some((name, false)) = kind.dispatcher.clone() else {
                continue;
            };
            kind.dispatcher = Some((name.clone(), true));
            let mut function = dispatcher(&name, kind, type_id, span);
            let arguments: Vec<Expr> = (0..kind.parameters.len()).map(|index| Expr::Name(format!("${index}"), span)).collect();
            let call = |member: &String| Statement::Return {
                value: Some(Expr::Call { name: member.clone(), type_arguments: Vec::new(), arguments: arguments.clone(), span }),
                span,
            };
            let count = kind.members.len();
            let arms = kind.members.iter().enumerate().map(|(index, member)| MatchArm {
                // The last is any value, so that the match is complete.
                pattern: if index + 1 == count { Pattern::Wildcard(span) } else { Pattern::Literal(Expr::Integer(index as i64, span)) },
                body: vec![call(member)],
                span,
            });
            let subject = Expr::Conversion { target: TypeName::U16, value: Box::new(Expr::Name("$function".into(), span)), span };
            function.body = if count == 0 {
                // No function has this type: no value of it exists to call.
                vec![Statement::While { condition: Expr::Boolean(true, span), body: vec![Statement::Continue(span)], span }]
            } else {
                vec![Statement::Match { subject, arms: arms.collect(), span }]
            };
            built.push(function);
        }
        built
    }
}

/// `fn(A) -> R` for these parameters and result.
fn spelled(parameters: &[ParameterType], result: &TypeAnnotation) -> TypeSpec {
    let args = parameters
        .iter()
        .map(|one| match one {
            ParameterType::Owned(annotation) => annotation.clone(),
            ParameterType::Borrowed { mutable, target } => TypeAnnotation::Value(TypeSpec::Applied {
                name: if *mutable { "&mut" } else { "&" }.into(),
                args: vec![target.clone()],
            }),
        })
        .chain([result.clone()])
        .collect();
    TypeSpec::Applied { name: FUNCTION.into(), args }
}

/// A dispatcher's header: the value, then the type's parameters.
fn dispatcher(name: &str, kind: &FunctionType, type_id: u32, span: Span) -> Function {
    let value = Parameter {
        name: "$function".into(),
        type_: ParameterType::Owned(TypeAnnotation::Value(TypeSpec::Primitive(TypeName::Function { type_id }))),
        default: None,
        span,
    };
    let parameters = kind.parameters.iter().enumerate().map(|(index, one)| Parameter {
        name: format!("${index}"),
        type_: one.clone(),
        default: None,
        span,
    });
    Function {
        name: name.into(),
        generics: Vec::new(),
        parameters: std::iter::once(value).chain(parameters).collect(),
        result: kind.result.clone(),
        body: Vec::new(),
        span,
    }
}

impl FunctionCompiler<'_> {
    /// The function `name` as a value of its type, if it names a function.
    pub(super) fn function_value(&mut self, name: &str, expected: Option<TypeName>, span: Span) -> Option<Result<TypedOperand, Diagnostic>> {
        let signature = self.signatures.get(name)?.clone();
        if signature.abi.interrupt() {
            return Some(self.foreign_address(&signature, expected, span));
        }
        let type_name = self.types.function_type(&signature);
        if let Some(wanted) = expected.filter(|one| *one != type_name) {
            return Some(Err(type_mismatch(span, wanted, type_name)));
        }
        let index = self.types.member(type_name, name);
        Some(Ok(TypedOperand { operand: Some(hir::Operand::Constant(type_id(type_name), index)), type_name }))
    }

    /// The type of the function `name` names, as a value, if one was made.
    pub(super) fn function_value_hint(&self, name: &str) -> Option<TypeName> {
        let signature = self.signatures.get(name)?;
        let function = self.types.function_type_of(signature)?;
        if signature.abi.interrupt() {
            return self.types.foreign_function_of(signature.abi, function);
        }
        Some(function)
    }

    /// A lambda as a value of the function type `type_name`: a function of
    /// its own, which therefore captures nothing (section 4).
    pub(super) fn lifted_lambda(
        &mut self,
        parameters: &[LambdaParameter],
        body: &Expr,
        scopes: &[BTreeMap<String, Binding>],
        type_name: TypeName,
        span: Span,
    ) -> Result<TypedOperand, Diagnostic> {
        let TypeName::Function { type_id: kind } = type_name else {
            unreachable!("a function type")
        };
        let kind = self.types.function_types[&kind].clone();
        if parameters.len() != kind.parameters.len() {
            return Err(Diagnostic::new(span, format!("the lambda takes {} arguments; {} takes {}", parameters.len(), type_name_text(type_name), kind.parameters.len())));
        }
        let lambda = Expr::Lambda { parameters: parameters.to_vec(), body: Box::new(body.clone()), span };
        if let Some(name) = lambdas::captured(&lambda, scopes) {
            return Err(Diagnostic::new(span, format!("a lambda made a function value cannot capture {name:?}")));
        }
        let name = format!("{}.{}", self.signature.name, self.hidden("lambda"));
        let function = Function {
            name: name.clone(),
            generics: Vec::new(),
            parameters: parameters
                .iter()
                .zip(&kind.parameters)
                .map(|(one, type_)| Parameter { name: one.name.clone(), type_: type_.clone(), default: None, span })
                .collect(),
            result: kind.result.clone(),
            body: vec![Statement::Return { value: Some(body.clone()), span }],
            span,
        };
        self.templates.borrow_mut().generated(function, self.types)?;
        let index = self.types.member(type_name, &name);
        Ok(TypedOperand { operand: Some(hir::Operand::Constant(type_id(type_name), index)), type_name })
    }

    /// `name(arguments)` where `name` holds a function value: a call of its
    /// type's dispatcher, which is declared when first called.
    pub(super) fn dispatched_call(&mut self, name: &str, arguments: &[Expr], span: Span) -> Result<Option<Expr>, Diagnostic> {
        let Some(Binding { type_: BindingType::Scalar(TypeName::Function { type_id: kind }), .. }) = self.visible(name) else {
            return Ok(None);
        };
        let kind = *kind;
        let dispatcher_name = match &self.types.function_types[&kind].dispatcher {
            Some((declared, _)) => declared.clone(),
            None => {
                let declared = format!("$call{kind}");
                let header = dispatcher(&declared, &self.types.function_types[&kind], kind, span);
                self.templates.borrow_mut().declared(header, self.types)?;
                self.types.function_types.get_mut(&kind).expect("registered").dispatcher = Some((declared.clone(), false));
                declared
            }
        };
        let arguments = std::iter::once(Expr::Name(name.into(), span)).chain(arguments.iter().cloned()).collect();
        Ok(Some(Expr::Call { name: dispatcher_name, type_arguments: Vec::new(), arguments, span }))
    }
}
