//! Types, uniqued by the context as LLVM's are, so a type is a copyable id.

use std::collections::HashMap;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TypeId(u32);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum FloatKind {
    Float,
    Double,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum Type {
    Void,
    Label,
    Metadata,
    Token,
    Int(u32),
    Float(FloatKind),
    /// Opaque, in an address space.
    Pointer(u32),
    Array { element: TypeId, count: u64 },
    Vector { element: TypeId, count: u32 },
    /// A literal struct: identified by its fields.
    Struct { fields: Vec<TypeId>, packed: bool },
    /// An identified struct: `%name`, its body in the context.
    Named(String),
    Function { returns: TypeId, parameters: Vec<TypeId>, variadic: bool },
}

/// An identified struct's body; `None` while opaque.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StructBody {
    pub fields: Vec<TypeId>,
    pub packed: bool,
}

#[derive(Clone, Debug, Default)]
pub struct Types {
    types: Vec<Type>,
    interned: HashMap<Type, TypeId>,
    /// Identified structs in definition order.
    pub named: Vec<(String, Option<StructBody>)>,
}

impl Types {
    pub fn intern(&mut self, ty: Type) -> TypeId {
        if let Some(&id) = self.interned.get(&ty) {
            return id;
        }
        let id = TypeId(self.types.len() as u32);
        self.types.push(ty.clone());
        self.interned.insert(ty, id);
        id
    }

    pub fn get(&self, id: TypeId) -> &Type {
        &self.types[id.0 as usize]
    }

    pub fn void(&mut self) -> TypeId {
        self.intern(Type::Void)
    }

    pub fn int(&mut self, bits: u32) -> TypeId {
        self.intern(Type::Int(bits))
    }

    pub fn ptr(&mut self, space: u32) -> TypeId {
        self.intern(Type::Pointer(space))
    }

    pub fn int_bits(&self, id: TypeId) -> Option<u32> {
        match self.get(id) {
            Type::Int(bits) => Some(*bits),
            _ => None,
        }
    }

    pub fn is_void(&self, id: TypeId) -> bool {
        matches!(self.get(id), Type::Void)
    }

    pub fn body(&self, name: &str) -> Option<&StructBody> {
        self.named.iter().find(|(one, _)| one == name).and_then(|(_, body)| body.as_ref())
    }

    /// A struct's fields, literal or identified.
    pub fn fields(&self, id: TypeId) -> Option<&[TypeId]> {
        match self.get(id) {
            Type::Struct { fields, .. } => Some(fields),
            Type::Named(name) => self.body(name).map(|body| body.fields.as_slice()),
            _ => None,
        }
    }

    /// The type an aggregate's `index`th member has.
    pub fn member(&self, id: TypeId, index: u64) -> Option<TypeId> {
        match self.get(id) {
            Type::Array { element, count } => (index < *count).then_some(*element),
            Type::Vector { element, count } => (index < u64::from(*count)).then_some(*element),
            _ => self.fields(id)?.get(index as usize).copied(),
        }
    }

    /// LLVM's spelling.
    pub fn display(&self, id: TypeId) -> String {
        let list = |ids: &[TypeId]| ids.iter().map(|&one| self.display(one)).collect::<Vec<_>>().join(", ");
        match self.get(id) {
            Type::Void => "void".to_owned(),
            Type::Label => "label".to_owned(),
            Type::Metadata => "metadata".to_owned(),
            Type::Token => "token".to_owned(),
            Type::Int(bits) => format!("i{bits}"),
            Type::Float(FloatKind::Float) => "float".to_owned(),
            Type::Float(FloatKind::Double) => "double".to_owned(),
            Type::Pointer(0) => "ptr".to_owned(),
            Type::Pointer(space) => format!("ptr addrspace({space})"),
            Type::Array { element, count } => format!("[{count} x {}]", self.display(*element)),
            Type::Vector { element, count } => format!("<{count} x {}>", self.display(*element)),
            Type::Struct { fields, packed } => struct_text(&list(fields), *packed),
            Type::Named(name) => format!("%{}", crate::print::quoted(name)),
            Type::Function { returns, parameters, variadic } => {
                let dots = match (*variadic, parameters.is_empty()) {
                    (false, _) => "",
                    (true, true) => "...",
                    (true, false) => ", ...",
                };
                format!("{} ({}{dots})", self.display(*returns), list(parameters))
            }
        }
    }
}

/// `{ a, b }`, `<{ a, b }>`, or `{}` with nothing inside.
pub fn struct_text(fields: &str, packed: bool) -> String {
    let inner = if fields.is_empty() { "{}".to_owned() } else { format!("{{ {fields} }}") };
    if packed { format!("<{inner}>") } else { inner }
}
