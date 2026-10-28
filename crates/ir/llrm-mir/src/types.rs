//! Types, uniqued by the context as LLVM's are, so a type is a copyable id.

use std::cell::{Cell, RefCell};

use crate::arena::Arena;
use crate::hash::HashMap;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TypeId(u32);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum FloatKind {
    Float,
    Double,
    /// x87's 80-bit extended format, 10 bytes in memory. A constant of it
    /// holds the bits of the double nearest its value.
    X86Fp80,
}

impl FloatKind {
    /// The bytes a value of the kind stores (x87's 80 bits: 10), not the stride
    /// of an array of them, which is the
    /// target's (`Layout::long_double_bytes`).
    pub fn value_bytes(self) -> u32 {
        crate::datalayout::float_bits(self).div_ceil(8)
    }
}

/// The 10 bytes of an x87 extended value, from its 80 bits.
pub fn x87_image(bits: u128) -> [u8; 10] {
    let mut bytes = [0; 10];
    bytes.copy_from_slice(&bits.to_le_bytes()[..10]);
    bytes
}

/// The 10 bytes of x87's extended format holding the double whose bits
/// are `double`: its sign and exponent rebiased to 15 bits, and its fraction
/// under an explicit integer bit.
pub fn x87_extended(double: u64) -> [u8; 10] {
    let sign = (double >> 63) as u16;
    let exponent = ((double >> 52) & 0x7FF) as i32;
    let fraction = double & ((1 << 52) - 1);
    let (exponent, mantissa) = match (exponent, fraction) {
        (0, 0) => (0, 0),
        // A subnormal double is a normal extended one.
        (0, _) => {
            let shift = fraction.leading_zeros() - 11;
            ((1 - 1023 + 16383 - shift as i32) as u16, (fraction << shift) << 11)
        }
        (0x7FF, _) => (0x7FFF, (1 << 63) | (fraction << 11)),
        _ => ((exponent - 1023 + 16383) as u16, (1 << 63) | (fraction << 11)),
    };
    let mut bytes = [0; 10];
    bytes[..8].copy_from_slice(&mantissa.to_le_bytes());
    bytes[8..].copy_from_slice(&((sign << 15) | exponent).to_le_bytes());
    bytes
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
    Array {
        element: TypeId,
        count: u64,
    },
    Vector {
        element: TypeId,
        count: u32,
    },
    /// A literal struct: identified by its fields.
    Struct {
        fields: Vec<TypeId>,
        packed: bool,
    },
    /// An identified struct: `%name`, its body in the context.
    Named(String),
    Function {
        returns: TypeId,
        parameters: Vec<TypeId>,
        variadic: bool,
    },
}

/// An identified struct's body; `None` while opaque.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StructBody {
    pub fields: Vec<TypeId>,
    pub packed: bool,
}

#[derive(Clone, Debug, Default)]
pub struct Types {
    types: Arena<Type>,
    interned: Box<RefCell<HashMap<Type, TypeId>>>,
    /// `Type::Int(w)` for `w <= SMALL_INT`, the hot ask, without hashing.
    small_ints: SmallInts,
    /// Identified structs in definition order.
    pub named: Vec<(String, Option<StructBody>)>,
}

/// Widths with a direct slot.
const SMALL_INT: usize = 64;

/// A type id plus one per width, 0 where the width was never asked.
#[derive(Clone, Debug)]
struct SmallInts([Cell<u32>; SMALL_INT + 1]);

impl Default for SmallInts {
    fn default() -> Self {
        Self(std::array::from_fn(|_| Cell::new(0)))
    }
}

impl Types {
    /// LLVM's `IntegerType::get` and kin: `ty`, made on the first ask. The
    /// table is borrowed for the lookup and again for the insert, never across
    /// a call.
    #[inline]
    pub fn intern(
        &self,
        ty: Type,
    ) -> TypeId {
        let slot = match ty {
            Type::Int(bits) => self.small_ints.0.get(bits as usize),
            _ => None,
        };
        if let Some(slot) = slot {
            if slot.get() != 0 {
                return TypeId(slot.get() - 1);
            }
        }
        let found = self.interned.borrow().get(&ty).copied();
        let id = found.unwrap_or_else(|| {
            let id = TypeId(self.types.push(ty.clone()) as u32);
            self.interned.borrow_mut().insert(ty, id);
            id
        });
        if let Some(slot) = slot {
            slot.set(id.0 + 1);
        }
        id
    }

    #[inline]
    pub fn get(
        &self,
        id: TypeId,
    ) -> &Type {
        self.types.get(id.0 as usize)
    }

    /// `ty` of `from`, interned here: none for an identified struct, whose
    /// body is `from`'s own.
    pub fn imported(
        &self,
        from: &Types,
        ty: TypeId,
    ) -> Option<TypeId> {
        let one = match from.get(ty).clone() {
            Type::Named(_) => return None,
            Type::Array { element, count } => Type::Array { element: self.imported(from, element)?, count },
            Type::Vector { element, count } => Type::Vector { element: self.imported(from, element)?, count },
            Type::Struct { fields, packed } => Type::Struct {
                fields: fields.iter().map(|&one| self.imported(from, one)).collect::<Option<_>>()?,
                packed,
            },
            Type::Function { returns, parameters, variadic } => Type::Function {
                returns: self.imported(from, returns)?,
                parameters: parameters.iter().map(|&one| self.imported(from, one)).collect::<Option<_>>()?,
                variadic,
            },
            other => other,
        };
        Some(self.intern(one))
    }

    pub fn void(&self) -> TypeId {
        self.intern(Type::Void)
    }

    pub fn int(
        &self,
        bits: u32,
    ) -> TypeId {
        self.intern(Type::Int(bits))
    }

    pub fn ptr(
        &self,
        space: u32,
    ) -> TypeId {
        self.intern(Type::Pointer(space))
    }

    #[inline]
    pub fn int_bits(
        &self,
        id: TypeId,
    ) -> Option<u32> {
        match self.get(id) {
            Type::Int(bits) => Some(*bits),
            _ => None,
        }
    }

    #[inline]
    pub fn is_void(
        &self,
        id: TypeId,
    ) -> bool {
        matches!(self.get(id), Type::Void)
    }

    /// What a held fact about these types was true of: how many there are, and
    /// how many identified structs.
    pub fn version(&self) -> (usize, usize) {
        (self.types.len(), self.named.len())
    }

    pub fn body(
        &self,
        name: &str,
    ) -> Option<&StructBody> {
        self.named.iter().find(|(one, _)| one == name).and_then(|(_, body)| body.as_ref())
    }

    /// A struct's fields, literal or identified.
    pub fn fields(
        &self,
        id: TypeId,
    ) -> Option<&[TypeId]> {
        match self.get(id) {
            Type::Struct { fields, .. } => Some(fields),
            Type::Named(name) => self.body(name).map(|body| body.fields.as_slice()),
            _ => None,
        }
    }

    /// The type an aggregate's `index`th member has.
    pub fn member(
        &self,
        id: TypeId,
        index: u64,
    ) -> Option<TypeId> {
        match self.get(id) {
            Type::Array { element, count } => (index < *count).then_some(*element),
            Type::Vector { element, count } => (index < u64::from(*count)).then_some(*element),
            _ => self.fields(id)?.get(index as usize).copied(),
        }
    }

    /// LLVM's spelling.
    pub fn display(
        &self,
        id: TypeId,
    ) -> String {
        let list = |ids: &[TypeId]| ids.iter().map(|&one| self.display(one)).collect::<Vec<_>>().join(", ");
        match self.get(id) {
            Type::Void => "void".to_owned(),
            Type::Label => "label".to_owned(),
            Type::Metadata => "metadata".to_owned(),
            Type::Token => "token".to_owned(),
            Type::Int(bits) => format!("i{bits}"),
            Type::Float(FloatKind::Float) => "float".to_owned(),
            Type::Float(FloatKind::Double) => "double".to_owned(),
            Type::Float(FloatKind::X86Fp80) => "x86_fp80".to_owned(),
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
pub fn struct_text(
    fields: &str,
    packed: bool,
) -> String {
    let inner = if fields.is_empty() { "{}".to_owned() } else { format!("{{ {fields} }}") };
    if packed { format!("<{inner}>") } else { inner }
}

#[cfg(test)]
#[path = "types_tests.rs"]
mod tests;
