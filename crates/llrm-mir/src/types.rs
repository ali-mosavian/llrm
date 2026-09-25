//! Types, interned by the context so a value's type is a copyable handle.

use std::collections::HashMap;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TypeId(u32);

/// An exact semantic format; equal storage size does not make two equivalent.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum FloatFormat {
    Binary32,
    Binary64,
    Extended80,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum Type {
    Void,
    /// Signless: operations state signedness.
    Int(u32),
    Float(FloatFormat),
    /// Opaque: an address space, and no width or encoding.
    Pointer(String),
    Vector { element: TypeId, elements: u32 },
    Array { element: TypeId, elements: u32 },
    Struct { fields: Vec<TypeId>, name: Option<String> },
    Function { parameters: Vec<TypeId>, returns: Vec<TypeId>, variadic: bool },
}

/// The widest integer the interpreter carries.
pub const MAX_INT_BITS: u32 = 128;

#[derive(Clone, Debug)]
pub struct MirContext {
    types: Vec<Type>,
    interned: HashMap<Type, TypeId>,
}

impl Default for MirContext {
    fn default() -> Self {
        Self::new()
    }
}

impl MirContext {
    pub fn new() -> Self {
        let mut context = Self { types: Vec::new(), interned: HashMap::new() };
        context.int(1);
        context
    }

    /// `i1`, the condition type, which every context holds.
    pub fn bool(&self) -> TypeId {
        TypeId(0)
    }

    pub fn intern(&mut self, ty: Type) -> TypeId {
        if let Some(&id) = self.interned.get(&ty) {
            return id;
        }
        let id = TypeId(self.types.len() as u32);
        self.types.push(ty.clone());
        self.interned.insert(ty, id);
        id
    }

    pub fn int(&mut self, bits: u32) -> TypeId {
        self.intern(Type::Int(bits))
    }

    pub fn get(&self, id: TypeId) -> &Type {
        &self.types[id.0 as usize]
    }

    /// An integer type's width, `None` for every other type.
    pub fn int_bits(&self, id: TypeId) -> Option<u32> {
        match self.get(id) {
            Type::Int(bits) => Some(*bits),
            _ => None,
        }
    }

    pub fn display(&self, id: TypeId) -> String {
        let list = |ids: &[TypeId]| ids.iter().map(|&one| self.display(one)).collect::<Vec<_>>().join(", ");
        match self.get(id) {
            Type::Void => "void".to_owned(),
            Type::Int(bits) => format!("i{bits}"),
            Type::Float(FloatFormat::Binary32) => "f32".to_owned(),
            Type::Float(FloatFormat::Binary64) => "f64".to_owned(),
            Type::Float(FloatFormat::Extended80) => "f80".to_owned(),
            Type::Pointer(space) => format!("ptr({space})"),
            Type::Vector { element, elements } => format!("<{elements} x {}>", self.display(*element)),
            Type::Array { element, elements } => format!("[{elements} x {}]", self.display(*element)),
            Type::Struct { fields, name: None } => format!("{{{}}}", list(fields)),
            Type::Struct { fields, name: Some(name) } => format!("{name}{{{}}}", list(fields)),
            Type::Function { parameters, returns, variadic } => {
                let dots = match (*variadic, parameters.is_empty()) {
                    (false, _) => "",
                    (true, true) => "...",
                    (true, false) => ", ...",
                };
                format!("fn({}{dots}){}", list(parameters), returns_suffix(self, returns))
            }
        }
    }
}

/// ` -> T`, ` -> (T, U)`, or nothing for no returns.
pub fn returns_suffix(context: &MirContext, returns: &[TypeId]) -> String {
    match returns {
        [] => String::new(),
        [one] => format!(" -> {}", context.display(*one)),
        many => format!(" -> ({})", many.iter().map(|&one| context.display(one)).collect::<Vec<_>>().join(", ")),
    }
}
