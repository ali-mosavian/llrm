use super::*;
use crate::types::Types;

fn function(
    types: &mut Types,
    returns: TypeId,
    parameters: Vec<TypeId>,
    variadic: bool,
) -> TypeId {
    types.intern(Type::Function { returns, parameters, variadic })
}

fn generic(int_bits: u32) -> TargetLibraryInfo {
    TargetLibraryInfo::new(&Generic { int_bits, size_t_bits: int_bits })
}

/// The table has the pin's functions by their standard names: `memchr` and
/// `_Exit` (the one name the enum spells apart from `exit`).
#[test]
fn the_table_finds_a_function_by_its_standard_name() {
    assert_eq!(TargetLibraryInfo::get_lib_func_by_name("memchr"), Some(LibFunc::Memchr));
    assert_eq!(TargetLibraryInfo::get_lib_func_by_name("_Exit"), Some(LibFunc::UnderExit));
    assert_eq!(TargetLibraryInfo::get_lib_func_by_name("exit"), Some(LibFunc::Exit));
    assert_eq!(TargetLibraryInfo::get_lib_func_by_name("\u{1}memchr"), Some(LibFunc::Memchr));
    assert_eq!(TargetLibraryInfo::get_lib_func_by_name("not_a_libfunc"), None);
    assert_eq!(TargetLibraryInfo::get_lib_func_by_name(""), None);
    assert_eq!(ALL.len(), NUM_LIB_FUNCS);
    assert!(ALL.iter().enumerate().all(|(at, &func)| func as usize == at));
}

/// A declaration is a library function only with the prototype the table
/// states: `memchr(ptr, int, size_t) -> ptr`, `int` as wide as the target's.
#[test]
fn a_declaration_is_a_library_function_with_its_prototype() {
    let mut types = Types::default();
    let (ptr, i16, i32, i64, double) =
        (types.ptr(0), types.int(16), types.int(32), types.int(64), types.intern(Type::Float(FloatKind::Double)));
    let memchr32 = function(&mut types, ptr, vec![ptr, i32, i32], false);
    let memchr16 = function(&mut types, ptr, vec![ptr, i16, i16], false);
    let wrong = function(&mut types, ptr, vec![ptr, double, i32], false);
    let variadic = function(&mut types, ptr, vec![ptr, i32, i32], true);
    let wide = function(&mut types, ptr, vec![ptr, i32, i64], false);
    let (on32, on16) = (generic(32), generic(16));
    assert_eq!(on32.get_lib_func(&types, "memchr", memchr32), Some(LibFunc::Memchr));
    assert_eq!(on32.get_lib_func(&types, "memchr", memchr16), None, "int is 32 bits here");
    assert_eq!(on16.get_lib_func(&types, "memchr", memchr16), Some(LibFunc::Memchr));
    assert_eq!(on32.get_lib_func(&types, "memchr", wrong), None);
    assert_eq!(on32.get_lib_func(&types, "memchr", variadic), None);
    assert_eq!(on32.get_lib_func(&types, "memchr", wide), None, "size_t is 32 bits here");
}

/// The ellipsis ends a prototype and asks for a variadic function: `printf`.
#[test]
fn an_ellipsis_in_the_table_asks_for_a_variadic_declaration() {
    let mut types = Types::default();
    let (ptr, i32) = (types.ptr(0), types.int(32));
    let fixed = function(&mut types, i32, vec![ptr], false);
    let variadic = function(&mut types, i32, vec![ptr], true);
    let info = generic(32);
    assert_eq!(info.get_lib_func(&types, "printf", variadic), Some(LibFunc::Printf));
    assert_eq!(info.get_lib_func(&types, "printf", fixed), None);
}

/// `Same` asks the type of the one before: `fmax(double, double) -> double`.
#[test]
fn same_asks_the_type_before_it() {
    let mut types = Types::default();
    let (double, float) = (types.intern(Type::Float(FloatKind::Double)), types.intern(Type::Float(FloatKind::Float)));
    let right = function(&mut types, double, vec![double, double], false);
    let mixed = function(&mut types, double, vec![double, float], false);
    let info = generic(32);
    assert_eq!(info.get_lib_func(&types, "fmax", right), Some(LibFunc::Fmax));
    assert_eq!(info.get_lib_func(&types, "fmax", mixed), None);
}

/// A target that states nothing has the base set missing; one that states
/// more has it, and a function it calls by another name has that name.
#[test]
fn availability_is_what_the_target_description_states() {
    struct Runtime;
    impl Library for Runtime {
        fn int_bits(&self) -> u32 {
            16
        }

        fn size_t_bits(&self) -> u32 {
            16
        }

        fn availability(
            &self,
            func: LibFunc,
        ) -> Availability {
            match func {
                LibFunc::Strlen => Availability::Named("_fstrlen"),
                LibFunc::Memchr => Availability::Unavailable,
                _ => Availability::Standard,
            }
        }
    }
    let generic = generic(32);
    assert!(generic.has(LibFunc::Memchr) && !generic.has(LibFunc::Bcmp) && !generic.has(LibFunc::GetcUnlocked));
    assert_eq!(generic.name(LibFunc::Memchr), Some("memchr"));
    let mut dos = TargetLibraryInfo::new(&Runtime);
    assert_eq!((dos.has(LibFunc::Memchr), dos.name(LibFunc::Memchr)), (false, None));
    assert_eq!(dos.name(LibFunc::Strlen), Some("_fstrlen"));
    assert_eq!(dos.int_bits(), 16);
    dos.disable_all_functions();
    assert!(!dos.has(LibFunc::Strlen));
}

/// The functions the pin tests apart from the table are none here.
#[test]
fn a_special_prototype_is_not_recognized() {
    let mut types = Types::default();
    let double = types.intern(Type::Float(FloatKind::Double));
    let cabs = function(&mut types, double, vec![double, double], false);
    assert_eq!(generic(32).get_lib_func(&types, "cabs", cabs), None);
}
