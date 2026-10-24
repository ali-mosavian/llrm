//! DIM, REDIM, STATIC, COMMON and SHARED: what a name is declared to be, and
//! what conflicts.

use super::*;

impl Checker {
    pub(super) fn declare(
        &mut self,
        d: &Declaration,
        redim: bool,
    ) {
        let span = d.span;
        if self.reserved_name(&d.name, span) {
            return;
        }
        if let Some((prefix, _)) = d.name.split_once('.') {
            // `r.a` names a field of the record variable `r`; a record
            // variable's dotted names are not free
            let record = |scope: &Scope| {
                scope.vars.get(stem(prefix)).is_some_and(|vars| vars.iter().any(|v| matches!(v.ty, Ty::Record(_))))
            };
            if record(&self.scope) || record(&self.module_scope) {
                self.err(span, "Duplicate definition");
                return;
            }
            if d.type_name.is_none() && matches!(
                self.dialect,
                Dialect::Pds71 | Dialect::VbDos | Dialect::Quickr
            ) {
                self.err(span, "AS clause required");
                return;
            }
        }
        let mut ty = match d.type_name.as_ref() {
            Some(name) => match self.type_of_name(name) {
                Some(ty) => ty,
                None => {
                    self.err(span, "User-defined type not defined");
                    return;
                }
            },
            None => self.default_type(&d.name),
        };
        if let (Ty::Str(_), Some(length)) = (&ty, &d.fixed_length) {
            if let Some(typed) = self.constant_expr(length) {
                ty = Ty::Str(typed.constant.map(|c| c as usize));
            }
        }
        if d.bounds.len() > 60 {
            self.err(span, "Too many dimensions");
            return;
        }
        let mut constant_bounds = true;
        for bound in &d.bounds {
            if let Some(lower) = &bound.lower {
                constant_bounds &= self.integer(lower).is_some_and(|typed| typed.constant.is_some());
            }
            constant_bounds &= self.integer(&bound.upper).is_some_and(|typed| typed.constant.is_some());
        }
        let key = stem(&d.name).to_owned();
        let as_declared = d.type_name.is_some() && Ty::of_suffix(&d.name).is_none();
        if !d.array && self.consts.contains_key(&key) {
            self.err(span, "Duplicate definition");
            return;
        }
        let local_existing: Vec<Var> = self.scope.vars.get(&key).cloned().unwrap_or_default();
        let mut existing = local_existing.clone();
        // a DIM in a procedure meets what the module shares (a STATIC does not;
        // VBDOS lets a scalar shadow it)
        if self.procedure.is_some()
            && !self.static_statement
            && (d.array || !matches!(self.dialect, Dialect::VbDos | Dialect::Quickr))
        {
            if let Some(shared) = self.module_scope.vars.get(&key) {
                for var in shared.iter().filter(|v| v.shared) {
                    if !existing.iter().any(|v| v.rank == var.rank && v.ty.plain() == var.ty.plain() && v.shared) {
                        existing.push(var.clone());
                    }
                }
            }
        }
        // a scalar cannot be named like a procedure
        if !d.array && self.procs.get(&key).is_some_and(|p| !p.def_fn) {
            self.err(span, "Name of subprogram illegal");
            return;
        }
        // PDS and VBDOS: AS binds a name only when nothing of that type was
        // named without it
        if as_declared && !matches!(self.dialect, Dialect::QBasic11 | Dialect::QuickBasic45) {
            let named = local_existing.iter().any(|v| !v.declared && v.ty.plain() == ty.plain())
                || self.consts.get(&key).is_some_and(|c| c.ty.plain() == ty.plain());
            if named {
                self.err(span, "AS clause required on first declaration");
                return;
            }
        }
        if d.array {
            // an AS-bound array, a suffixed one and a bare one bind the stem as
            // scalars' names do
            if !existing.iter().any(|v| v.rank.is_some() && v.ty.plain() == ty.plain() && v.declared == as_declared) {
                if let Some(message) = self.scalar_conflict(&existing, &ty, &d.name, as_declared, true) {
                    self.err(span, message);
                    return;
                }
            }
            if let Some(old) =
                existing.iter().find(|v| v.rank.is_some() && v.ty.plain() == ty.plain() && v.declared == as_declared)
            {
                // `COMMON a() AS T` named the array; its DIM gives the bounds
                if old.rank == Some(usize::MAX) && !d.bounds.is_empty() {
                    if let Some(vars) = self.scope.vars.get_mut(&key) {
                        vars.retain(|v| v.rank != Some(usize::MAX));
                    }
                } else {
                    if (redim && old.dynamic) || (!redim && old.redimmed) {
                        // REDIM resizes a dynamic array
                        if old.ty.plain() != ty.plain() {
                            self.err(span, "Duplicate definition");
                        }
                        return;
                    }
                    self.err(span, "Array already dimensioned");
                    return;
                }
            }
        } else if let Some(message) = self.scalar_conflict(&existing, &ty, &d.name, as_declared, false) {
            self.err(span, message);
            return;
        }
        // a name bound by AS to another type, or a different suffix, is taken
        if !d.array && existing.iter().any(|v| v.declared && v.ty.plain() != ty.plain() && v.rank.is_none()) {
            self.err(span, "Duplicate definition");
            return;
        }
        let var = Var {
            ty,
            rank: d.array.then_some(if d.bounds.is_empty() { usize::MAX } else { d.bounds.len() }),
            declared: as_declared,
            shared: d.shared,
            dynamic: redim
                || d.dynamic
                || (d.array && d.bounds.is_empty())
                // variable bounds, or an automatic array in a procedure that is
                // not STATIC
                || (d.array && !constant_bounds)
                || (d.array && self.procedure.is_some() && !self.static_scope),
            explicit: true,
            suffixed: Ty::of_suffix(&d.name).is_some(),
            redimmed: redim,
        };
        self.scope.vars.entry(key).or_default().push(var);
    }

    /// Why declaring a scalar `name` of type `ty` clashes with the variables
    /// already named by its stem (BC's rules: `q` and `q%` are two variables,
    /// `q AS T` binds the stem, and PDS and VBDOS refuse to mix a suffix with
    /// AS).
    pub(super) fn scalar_conflict(
        &self,
        existing: &[Var],
        ty: &Ty,
        name: &str,
        as_declared: bool,
        arrays: bool,
    ) -> Option<&'static str> {
        let suffixed = Ty::of_suffix(name).is_some();
        let mixes = !matches!(self.dialect, Dialect::QBasic11 | Dialect::QuickBasic45);
        for v in existing.iter().filter(|v| v.rank.is_some() == arrays) {
            let bare = !v.declared && !v.suffixed;
            if as_declared {
                if mixes && v.suffixed && v.ty.plain() == ty.plain() {
                    return Some("AS clause required on first declaration");
                }
                if v.explicit && (v.declared || bare) {
                    return Some("Duplicate definition");
                }
            } else if suffixed {
                if mixes && v.declared && v.explicit && v.ty.plain() == ty.plain() {
                    return Some("AS clause required");
                }
                if v.explicit
                    && (v.declared
                        || (v.suffixed && v.ty.plain() == ty.plain())
                        || (bare && v.ty.plain() == ty.plain()))
                {
                    return Some("Duplicate definition");
                }
            } else {
                if v.explicit && v.declared {
                    return Some("AS clause required");
                }
                if v.explicit && (bare || (v.suffixed && v.ty.plain() == ty.plain())) {
                    return Some("Duplicate definition");
                }
            }
        }
        None
    }

    pub(super) fn share(
        &mut self,
        d: &Declaration,
    ) {
        let key = stem(&d.name).to_owned();
        if self.scope.vars.get(&key).is_some_and(|vars| vars.iter().any(|v| v.explicit && v.rank.is_none() == !d.array))
        {
            self.err(d.span, "Variable required");
            return;
        }
        let found = self.module_scope.vars.get(&key).cloned().unwrap_or_default();
        let wanted = d.type_name.as_ref().and_then(|name| self.type_of_name(name));
        let pick = found.into_iter().find(|v| v.rank.is_some() == d.array);
        match pick {
            Some(var) => {
                if wanted.as_ref().is_some_and(|w| w.plain() != var.ty.plain()) {
                    self.err(d.span, "Duplicate definition");
                    return;
                }
                // a record is shared by `SHARED r AS Type`
                if wanted.is_none() && matches!(var.ty, Ty::Record(_)) {
                    self.err(d.span, "AS clause required");
                    return;
                }
                self.scope.vars.entry(key).or_default().push(var);
            }
            None => {
                let ty = wanted.clone().unwrap_or_else(|| self.default_type(&d.name));
                let var = Var {
                    ty,
                    rank: d.array.then_some(usize::MAX),
                    declared: wanted.is_some() && Ty::of_suffix(&d.name).is_none(),
                    shared: true,
                    dynamic: false,
                    explicit: true,
                    suffixed: Ty::of_suffix(&d.name).is_some(),
                    redimmed: false,
                };
                self.scope.vars.entry(key.clone()).or_default().push(var.clone());
                self.module_scope.vars.entry(key).or_default().push(var);
            }
        }
    }
}
