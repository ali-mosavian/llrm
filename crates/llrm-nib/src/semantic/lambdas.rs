//! Lambdas: `|x| body` is compiled where it is called, in the scopes it was
//! written in, so it reads their bindings and costs no call. Passed to a
//! generic function, it becomes a local of that function's instance.

use super::*;
use crate::syntax::LambdaParameter;

#[derive(Clone, Debug)]
pub(super) struct Lambda {
    parameters: Vec<LambdaParameter>,
    body: Expr,
    /// The bindings it can see: those in scope where it was written.
    scopes: Vec<BTreeMap<String, Binding>>,
}

impl Lambda {
    /// This lambda as a value of the function type `type_name`.
    pub(super) fn lifted(&self, compiler: &mut FunctionCompiler<'_>, type_name: TypeName, span: Span) -> Result<TypedOperand, Diagnostic> {
        compiler.lifted_lambda(&self.parameters, &self.body, &self.scopes, type_name, span)
    }
}

impl FunctionCompiler<'_> {
    /// `let name = |x| ...`
    pub(super) fn bind_lambda(&mut self, name: &str, lambda: &Expr) {
        let binding = self.lambda_binding(lambda, self.scopes.clone());
        self.scopes
            .last_mut()
            .expect("scope")
            .insert(name.into(), binding);
    }

    /// A name for `lambda`, which sees `scopes`.
    pub(super) fn lambda_binding(
        &mut self,
        lambda: &Expr,
        scopes: Vec<BTreeMap<String, Binding>>,
    ) -> Binding {
        let Expr::Lambda {
            parameters, body, ..
        } = lambda
        else {
            unreachable!("a lambda")
        };
        let index = self.lambdas.len() as u32;
        self.lambdas.push(Lambda {
            parameters: parameters.clone(),
            body: (**body).clone(),
            scopes,
        });
        Binding {
            type_: BindingType::Scalar(TypeName::Void),
            mutable: false,
            storage: Storage::Lambda(index),
        }
    }

    /// The lambda a name is bound to.
    pub(super) fn lambda_named(&self, name: &str) -> Option<u32> {
        match self.visible(name)?.storage {
            Storage::Lambda(index) => Some(index),
            _ => None,
        }
    }

    /// `name(arguments)` for a lambda: its body, its parameters bound to the
    /// arguments, compiled in the scopes it was written in.
    pub(super) fn inline_lambda(
        &mut self,
        index: u32,
        arguments: &[Expr],
        expected: Option<TypeName>,
        span: Span,
    ) -> Result<TypedOperand, Diagnostic> {
        let lambda = self.lambdas[index as usize].clone();
        if arguments.len() != lambda.parameters.len() {
            return Err(Diagnostic::new(
                span,
                format!("the lambda takes {} arguments", lambda.parameters.len()),
            ));
        }
        let mut parameters = BTreeMap::new();
        for (parameter, argument) in lambda.parameters.iter().zip(arguments) {
            parameters.insert(
                parameter.name.clone(),
                self.lambda_argument(parameter, argument, span)?,
            );
        }
        let mut body = lambda.body.clone();
        self.prepare_expression(&mut body)?;
        let caller = std::mem::replace(&mut self.scopes, lambda.scopes);
        let hidden = std::mem::take(&mut self.hidden);
        self.scopes.push(parameters);
        let result = self.expression(&body, expected);
        self.scopes = caller;
        self.hidden = hidden;
        result
    }

    /// A lambda parameter bound to its argument, evaluated in the caller.
    fn lambda_argument(
        &mut self,
        parameter: &LambdaParameter,
        argument: &Expr,
        span: Span,
    ) -> Result<Binding, Diagnostic> {
        let declared = parameter
            .type_
            .as_ref()
            .map(|spec| self.types.resolve_element(spec, span))
            .transpose()?;
        let aggregate = match declared {
            Some(ElementType::Struct(id)) => Some(id),
            Some(ElementType::Scalar(_)) => None,
            None => self.struct_expression_type(argument, span)?,
        };
        if let Some(struct_id) = aggregate {
            let view = self.struct_view(argument, span)?;
            if view.struct_id != struct_id {
                return Err(Diagnostic::new(
                    argument.span(),
                    "the argument has the wrong struct type",
                ));
            }
            let pointer = self.address_of(&view);
            let hir::Operand::Value(pointer) = pointer else {
                unreachable!("an address is a value")
            };
            return Ok(Binding {
                type_: BindingType::Struct(struct_id),
                mutable: false,
                storage: Storage::Reference(pointer),
            });
        }
        let value = match declared {
            Some(ElementType::Scalar(type_name)) => self.coerced(argument, type_name)?,
            _ => self.expression(argument, None)?,
        };
        let type_name = value.type_name;
        let value = self.materialized(required(value, span)?, type_id(type_name));
        Ok(Binding {
            type_: BindingType::Scalar(type_name),
            mutable: false,
            storage: Storage::Parameter(value),
        })
    }

    /// The lambda an argument passes, written out, when it passes one.
    pub(super) fn lambda_argument_expression(
        &self,
        argument: &Expr,
    ) -> Option<(Expr, Vec<BTreeMap<String, Binding>>)> {
        match argument {
            Expr::Lambda { .. } => Some((argument.clone(), self.scopes.clone())),
            Expr::Name(name, span) => {
                let lambda = &self.lambdas[self.lambda_named(name)? as usize];
                let expression = Expr::Lambda {
                    parameters: lambda.parameters.clone(),
                    body: Box::new(lambda.body.clone()),
                    span: *span,
                };
                Some((expression, lambda.scopes.clone()))
            }
            _ => None,
        }
    }
}

/// The first name `lambda` reads from `scopes` rather than its parameters:
/// a lambda passed to a function may not capture.
pub(super) fn captured(lambda: &Expr, scopes: &[BTreeMap<String, Binding>]) -> Option<String> {
    let Expr::Lambda {
        parameters, body, ..
    } = lambda
    else {
        return None;
    };
    let mut body = (**body).clone();
    let mut found = None;
    let _ = body.walk_mut(&mut |one| {
        if let Expr::Name(name, _) = one {
            let local = scopes.iter().any(|scope| scope.contains_key(name.as_str()));
            if found.is_none() && local && !parameters.iter().any(|one| &one.name == name) {
                found = Some(name.clone());
            }
        }
        Ok::<(), ()>(())
    });
    found
}
