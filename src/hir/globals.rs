//! Exact planning of HIR static data and addressable places.
//!
//! This helper has no lowering side effects.  It identifies the portable IR
//! declarations and opaque pointer types needed to represent the narrow,
//! relocation-free static-data subset exactly.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use crate::{hir, ir};

/// Synthetic types, static data declarations, and place-address metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct GlobalPlan {
    /// Reused source types which static declarations require lowering.
    pub(super) source_types: BTreeSet<hir::TypeId>,
    /// Types not already represented by a source HIR type.
    pub(super) extra_types: Vec<ir::Type>,
    /// Static data declarations in source data-object order.
    pub(super) globals: Vec<ir::Global>,
    /// Addressable module and static places in deterministic source-ID order.
    pub(super) places: BTreeMap<(hir::FunctionId, hir::PlaceId), PlannedPlace>,
}

/// The portable global address associated with one HIR place.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct PlannedPlace {
    pub(super) global: ir::GlobalId,
    pub(super) pointer_type: ir::TypeId,
    pub(super) readonly: bool,
    /// Signed byte offset from the global's base address.
    pub(super) addend: i64,
}

/// A refusal raised while planning static data or a place address.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GlobalPlanError {
    UnsupportedStorage {
        function: hir::FunctionId,
        place: hir::PlaceId,
        storage: hir::Storage,
    },
    MissingData {
        function: hir::FunctionId,
        place: hir::PlaceId,
        data: hir::DataId,
    },
    AmbiguousData {
        data: hir::DataId,
    },
    Relocations {
        data: hir::DataId,
    },
    NegativePlaceOffset {
        function: hir::FunctionId,
        place: hir::PlaceId,
        offset: isize,
    },
    PlaceRangeOverflow {
        function: hir::FunctionId,
        place: hir::PlaceId,
    },
    PlaceOutOfBounds {
        function: hir::FunctionId,
        place: hir::PlaceId,
        data: hir::DataId,
    },
    AddressMismatch {
        function: hir::FunctionId,
        place: hir::PlaceId,
        data: hir::DataId,
        place_address: hir::AddressKind,
        data_address: hir::AddressKind,
    },
    DuplicatePlace {
        function: hir::FunctionId,
        place: hir::PlaceId,
    },
    LengthOverflow {
        data: hir::DataId,
    },
    AddendOverflow {
        function: hir::FunctionId,
        place: hir::PlaceId,
    },
    TypeIdOverflow {
        maximum: hir::TypeId,
    },
}

impl fmt::Display for GlobalPlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedStorage {
                function,
                place,
                storage,
            } => write!(
                formatter,
                "function {function} place {place} has unsupported {storage:?} storage"
            ),
            Self::MissingData {
                function,
                place,
                data,
            } => write!(
                formatter,
                "function {function} place {place} refers to missing data {data}"
            ),
            Self::AmbiguousData { data } => write!(formatter, "data id {data} is ambiguous"),
            Self::Relocations { data } => {
                write!(formatter, "data {data} has unsupported relocations")
            }
            Self::NegativePlaceOffset {
                function,
                place,
                offset,
            } => write!(
                formatter,
                "function {function} place {place} has negative offset {offset}"
            ),
            Self::PlaceRangeOverflow { function, place } => write!(
                formatter,
                "function {function} place {place} has an overflowing byte range"
            ),
            Self::PlaceOutOfBounds {
                function,
                place,
                data,
            } => write!(
                formatter,
                "function {function} place {place} exceeds data {data}"
            ),
            Self::AddressMismatch {
                function,
                place,
                data,
                place_address,
                data_address,
            } => write!(
                formatter,
                "function {function} place {place} address {place_address:?} does not match data {data} address {data_address:?}"
            ),
            Self::DuplicatePlace { function, place } => {
                write!(formatter, "function {function} repeats place {place}")
            }
            Self::LengthOverflow { data } => {
                write!(formatter, "data {data} length cannot be represented in IR")
            }
            Self::AddendOverflow { function, place } => write!(
                formatter,
                "function {function} place {place} offset cannot be represented in IR"
            ),
            Self::TypeIdOverflow { maximum } => write!(
                formatter,
                "cannot allocate a synthetic type after type id {maximum}"
            ),
        }
    }
}

impl Error for GlobalPlanError {}

/// Plans exact relocation-free HIR data and addressable static places.
pub(super) fn plan_globals(module: &hir::Module) -> Result<GlobalPlan, GlobalPlanError> {
    let data = DataTable::new(module);
    let mut referenced = BTreeSet::new();
    let mut pending_places = Vec::new();

    for function in &module.functions {
        for place in &function.places {
            if !matches!(place.storage, hir::Storage::Module | hir::Storage::Static) {
                return Err(GlobalPlanError::UnsupportedStorage {
                    function: function.id,
                    place: place.id,
                    storage: place.storage,
                });
            }
            let object = data.get(function.id, place.id, place.symbol)?;
            validate_place(function.id, place, object)?;
            referenced.insert(place.symbol);
            pending_places.push((function.id, place, object.readonly));
        }
    }

    let selected = module
        .data
        .iter()
        .filter(|object| {
            referenced.contains(&object.id)
                || object.linkage != hir::Linkage::Internal
                || !object.bytes.is_empty()
                || !object.relocations.is_empty()
        })
        .collect::<Vec<_>>();
    for object in &selected {
        if data.is_ambiguous(object.id) {
            return Err(GlobalPlanError::AmbiguousData { data: object.id });
        }
        if !object.relocations.is_empty() {
            return Err(GlobalPlanError::Relocations { data: object.id });
        }
    }

    if selected.is_empty() {
        return Ok(GlobalPlan {
            source_types: BTreeSet::new(),
            extra_types: Vec::new(),
            globals: Vec::new(),
            places: BTreeMap::new(),
        });
    }

    let mut allocator = TypeIdAllocator::new(module);
    let (i8, synthetic_i8) = match existing_i8(module) {
        Some(type_id) => (type_id, false),
        None => (allocator.allocate()?, true),
    };

    let lengths = selected
        .iter()
        .map(|object| {
            u64::try_from(object.bytes.len())
                .map_err(|_| GlobalPlanError::LengthOverflow { data: object.id })
        })
        .collect::<Result<BTreeSet<_>, _>>()?;
    let mut arrays = BTreeMap::new();
    for length in lengths {
        arrays.insert(length, allocator.allocate()?);
    }

    let existing_pointers = existing_pointers(module);
    let mut source_types = BTreeSet::new();
    if !synthetic_i8 {
        source_types.insert(hir::TypeId::new(i8.get()));
    }
    let addresses = pending_places
        .iter()
        .map(|(_, place, _)| place.address)
        .collect::<BTreeSet<_>>();
    let mut pointers = BTreeMap::new();
    let mut synthetic_pointers = BTreeSet::new();
    for address in addresses {
        if let Some(type_id) = existing_pointers.get(&address).copied() {
            pointers.insert(address, type_id);
            source_types.insert(hir::TypeId::new(type_id.get()));
        } else {
            pointers.insert(address, allocator.allocate()?);
            synthetic_pointers.insert(address);
        }
    }

    let mut extra_types = Vec::new();
    if synthetic_i8 {
        extra_types.push(ir::Type {
            id: i8,
            kind: ir::TypeKind::Integer { bits: 8 },
        });
    }
    for (length, type_id) in &arrays {
        extra_types.push(ir::Type {
            id: *type_id,
            kind: ir::TypeKind::Array {
                element: i8,
                length: *length,
            },
        });
    }
    for (address, type_id) in &pointers {
        if synthetic_pointers.contains(address) {
            extra_types.push(ir::Type {
                id: *type_id,
                kind: ir::TypeKind::Pointer {
                    address_space: address_space(*address),
                },
            });
        }
    }

    let mut globals = Vec::with_capacity(selected.len());
    for object in selected {
        let length = u64::try_from(object.bytes.len())
            .map_err(|_| GlobalPlanError::LengthOverflow { data: object.id })?;
        let type_id = arrays
            .get(&length)
            .copied()
            .ok_or(GlobalPlanError::LengthOverflow { data: object.id })?;
        globals.push(ir::Global {
            id: ir::GlobalId::new(object.id.get()),
            name: object.name.clone(),
            type_id,
            linkage: linkage(object.linkage),
            constant: object.readonly,
            initializer: Some(ir::Constant::Bytes(object.bytes.clone())),
        });
    }

    let mut places = BTreeMap::new();
    for (function, place, readonly) in pending_places {
        let pointer_type =
            pointers
                .get(&place.address)
                .copied()
                .ok_or(GlobalPlanError::PlaceRangeOverflow {
                    function,
                    place: place.id,
                })?;
        let addend = i64::try_from(place.offset).map_err(|_| GlobalPlanError::AddendOverflow {
            function,
            place: place.id,
        })?;
        if places
            .insert(
                (function, place.id),
                PlannedPlace {
                    global: ir::GlobalId::new(place.symbol.get()),
                    pointer_type,
                    readonly,
                    addend,
                },
            )
            .is_some()
        {
            return Err(GlobalPlanError::DuplicatePlace {
                function,
                place: place.id,
            });
        }
    }

    Ok(GlobalPlan {
        source_types,
        extra_types,
        globals,
        places,
    })
}

struct DataTable<'module> {
    objects: BTreeMap<hir::DataId, &'module hir::DataObject>,
    duplicates: BTreeSet<hir::DataId>,
}

impl<'module> DataTable<'module> {
    fn new(module: &'module hir::Module) -> Self {
        let mut objects = BTreeMap::new();
        let mut duplicates = BTreeSet::new();
        for object in &module.data {
            if objects.insert(object.id, object).is_some() {
                duplicates.insert(object.id);
            }
        }
        Self {
            objects,
            duplicates,
        }
    }

    fn get(
        &self,
        function: hir::FunctionId,
        place: hir::PlaceId,
        id: hir::DataId,
    ) -> Result<&'module hir::DataObject, GlobalPlanError> {
        if self.duplicates.contains(&id) {
            return Err(GlobalPlanError::AmbiguousData { data: id });
        }
        self.objects
            .get(&id)
            .copied()
            .ok_or(GlobalPlanError::MissingData {
                function,
                place,
                data: id,
            })
    }

    fn is_ambiguous(&self, id: hir::DataId) -> bool {
        self.duplicates.contains(&id)
    }
}

fn validate_place(
    function: hir::FunctionId,
    place: &hir::Place,
    object: &hir::DataObject,
) -> Result<(), GlobalPlanError> {
    if !object.relocations.is_empty() {
        return Err(GlobalPlanError::Relocations { data: object.id });
    }
    if place.offset < 0 {
        return Err(GlobalPlanError::NegativePlaceOffset {
            function,
            place: place.id,
            offset: place.offset,
        });
    }
    if place.address != object.address {
        return Err(GlobalPlanError::AddressMismatch {
            function,
            place: place.id,
            data: object.id,
            place_address: place.address,
            data_address: object.address,
        });
    }
    let start = u64::try_from(place.offset).map_err(|_| GlobalPlanError::PlaceRangeOverflow {
        function,
        place: place.id,
    })?;
    let extent = u64::try_from(place.extent).map_err(|_| GlobalPlanError::PlaceRangeOverflow {
        function,
        place: place.id,
    })?;
    let end = start
        .checked_add(extent)
        .ok_or(GlobalPlanError::PlaceRangeOverflow {
            function,
            place: place.id,
        })?;
    let length = u64::try_from(object.bytes.len())
        .map_err(|_| GlobalPlanError::LengthOverflow { data: object.id })?;
    if end > length {
        return Err(GlobalPlanError::PlaceOutOfBounds {
            function,
            place: place.id,
            data: object.id,
        });
    }
    Ok(())
}

struct TypeIdAllocator {
    maximum: Option<hir::TypeId>,
    next: Option<u32>,
    started: bool,
}

impl TypeIdAllocator {
    fn new(module: &hir::Module) -> Self {
        Self {
            maximum: module.types.iter().map(|type_| type_.id).max(),
            next: None,
            started: false,
        }
    }

    fn allocate(&mut self) -> Result<ir::TypeId, GlobalPlanError> {
        let raw = if self.started {
            self.next.ok_or(GlobalPlanError::TypeIdOverflow {
                maximum: self.maximum.unwrap_or(hir::TypeId::new(u32::MAX)),
            })?
        } else {
            self.started = true;
            match self.maximum {
                Some(maximum) => maximum
                    .get()
                    .checked_add(1)
                    .ok_or(GlobalPlanError::TypeIdOverflow { maximum })?,
                None => 0,
            }
        };
        self.next = raw.checked_add(1);
        Ok(ir::TypeId::new(raw))
    }
}

fn existing_i8(module: &hir::Module) -> Option<ir::TypeId> {
    module
        .types
        .iter()
        .filter(|type_| type_.kind == hir::TypeKind::Integer && type_.width == 1)
        .map(|type_| ir::TypeId::new(type_.id.get()))
        .min()
}

fn existing_pointers(module: &hir::Module) -> BTreeMap<hir::AddressKind, ir::TypeId> {
    let mut pointers = BTreeMap::new();
    for type_ in &module.types {
        if type_.kind != hir::TypeKind::Pointer {
            continue;
        }
        let type_id = ir::TypeId::new(type_.id.get());
        pointers
            .entry(type_.address)
            .and_modify(|existing| {
                if type_id < *existing {
                    *existing = type_id;
                }
            })
            .or_insert(type_id);
    }
    pointers
}

fn address_space(address: hir::AddressKind) -> ir::AddressSpace {
    match address {
        hir::AddressKind::None => ir::AddressSpace::Generic,
        hir::AddressKind::Near => ir::AddressSpace::NearData,
        hir::AddressKind::Far => ir::AddressSpace::FarData,
        hir::AddressKind::Huge => ir::AddressSpace::HugeData,
        hir::AddressKind::Code => ir::AddressSpace::Code,
        hir::AddressKind::Segment => ir::AddressSpace::Segment,
    }
}

fn linkage(linkage: hir::Linkage) -> ir::Linkage {
    match linkage {
        hir::Linkage::Internal => ir::Linkage::Internal,
        hir::Linkage::External => ir::Linkage::External,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VOID: hir::TypeId = hir::TypeId::new(0);

    fn type_(
        id: hir::TypeId,
        kind: hir::TypeKind,
        width: usize,
        address: hir::AddressKind,
    ) -> hir::Type {
        hir::Type {
            id,
            name: kind.as_str().into(),
            kind,
            width,
            signed: None,
            evaluation: hir::FloatEvaluation::None,
            element: None,
            bounds: Vec::new(),
            address,
        }
    }

    fn data(id: u32, bytes: Vec<u8>, address: hir::AddressKind) -> hir::DataObject {
        hir::DataObject {
            id: hir::DataId::new(id),
            name: format!("data{id}"),
            bytes,
            readonly: true,
            relocations: Vec::new(),
            linkage: hir::Linkage::Internal,
            address,
        }
    }

    fn place(
        id: u32,
        storage: hir::Storage,
        offset: isize,
        extent: usize,
        address: hir::AddressKind,
    ) -> hir::Place {
        hir::Place {
            id: hir::PlaceId::new(id),
            name: format!("place{id}"),
            type_id: VOID,
            storage,
            offset,
            symbol: hir::DataId::new(7),
            extent,
            address,
        }
    }

    fn module(data: Vec<hir::DataObject>, places: Vec<hir::Place>) -> hir::Module {
        hir::Module {
            id: hir::ModuleId::new(0),
            name: "globals".into(),
            types: vec![type_(VOID, hir::TypeKind::Void, 0, hir::AddressKind::None)],
            functions: vec![hir::Function {
                id: hir::FunctionId::new(0),
                name: "main".into(),
                result_type: VOID,
                values: Vec::new(),
                places,
                blocks: Vec::new(),
                entry: hir::BlockId::new(0),
                parameters: Vec::new(),
                abi: hir::ProcedureAbi {
                    cleanup: hir::StackCleanup::Callee,
                    distance: hir::CallDistance::Far,
                    parameter_bytes: 0,
                },
                calls: Vec::new(),
                error_handler: None,
                error_handler_local: false,
                external_entries: Vec::new(),
                linkage: hir::Linkage::Internal,
            }],
            data,
            callables: Vec::new(),
        }
    }

    #[test]
    fn plans_deterministic_types_and_shared_data_places() {
        let module = module(
            vec![data(7, vec![1, 2, 3], hir::AddressKind::Near)],
            vec![
                place(1, hir::Storage::Static, 0, 1, hir::AddressKind::Near),
                place(2, hir::Storage::Static, 1, 2, hir::AddressKind::Near),
            ],
        );

        let plan = plan_globals(&module).unwrap();

        assert_eq!(
            plan.extra_types,
            vec![
                ir::Type {
                    id: ir::TypeId::new(1),
                    kind: ir::TypeKind::Integer { bits: 8 },
                },
                ir::Type {
                    id: ir::TypeId::new(2),
                    kind: ir::TypeKind::Array {
                        element: ir::TypeId::new(1),
                        length: 3,
                    },
                },
                ir::Type {
                    id: ir::TypeId::new(3),
                    kind: ir::TypeKind::Pointer {
                        address_space: ir::AddressSpace::NearData,
                    },
                },
            ]
        );
        assert_eq!(plan.globals.len(), 1);
        assert_eq!(plan.globals[0].id, ir::GlobalId::new(7));
        assert_eq!(
            plan.globals[0].initializer,
            Some(ir::Constant::Bytes(vec![1, 2, 3]))
        );
        assert_eq!(plan.places.len(), 2);
        assert_eq!(
            plan.places[&(hir::FunctionId::new(0), hir::PlaceId::new(2))],
            PlannedPlace {
                global: ir::GlobalId::new(7),
                pointer_type: ir::TypeId::new(3),
                readonly: true,
                addend: 1,
            }
        );
    }

    #[test]
    fn refuses_relocated_data() {
        let mut object = data(7, vec![1], hir::AddressKind::Near);
        object.relocations.push(hir::DataRelocation {
            at: 0,
            target: hir::DataId::new(7),
            addend: 0,
            address: hir::AddressKind::Near,
        });
        let module = module(
            vec![object],
            vec![place(0, hir::Storage::Static, 0, 1, hir::AddressKind::Near)],
        );

        assert!(matches!(
            plan_globals(&module),
            Err(GlobalPlanError::Relocations { .. })
        ));
    }

    #[test]
    fn refuses_unsupported_storage() {
        let module = module(
            vec![data(7, vec![1], hir::AddressKind::Near)],
            vec![place(0, hir::Storage::Local, 0, 1, hir::AddressKind::Near)],
        );

        assert!(matches!(
            plan_globals(&module),
            Err(GlobalPlanError::UnsupportedStorage {
                storage: hir::Storage::Local,
                ..
            })
        ));
    }

    #[test]
    fn refuses_missing_data_and_out_of_bounds_places() {
        let missing = module(
            Vec::new(),
            vec![place(0, hir::Storage::Static, 0, 1, hir::AddressKind::Near)],
        );
        assert!(matches!(
            plan_globals(&missing),
            Err(GlobalPlanError::MissingData { .. })
        ));

        let bounds = module(
            vec![data(7, vec![1], hir::AddressKind::Near)],
            vec![place(0, hir::Storage::Static, 1, 1, hir::AddressKind::Near)],
        );
        assert!(matches!(
            plan_globals(&bounds),
            Err(GlobalPlanError::PlaceOutOfBounds { .. })
        ));
    }

    #[test]
    fn refuses_address_mismatches() {
        let module = module(
            vec![data(7, vec![1], hir::AddressKind::Far)],
            vec![place(0, hir::Storage::Module, 0, 1, hir::AddressKind::Near)],
        );

        assert!(matches!(
            plan_globals(&module),
            Err(GlobalPlanError::AddressMismatch { .. })
        ));
    }
}
