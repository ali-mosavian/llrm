use super::super::lexer::lex;
use crate::abi::modern as rt;
use super::super::parser::parse;

use super::*;

fn compile_source(source: &str) -> Result<String, Diagnostic> {
    super::super::compile(source, "test")
}

#[test]
fn emits_typed_cfg_for_loop_and_call() {
    let json = compile_source(
        "fn step(value: i16) -> i16:\n\
         \x20\x20\x20\x20return value + 1\n\
         fn count(limit: i16) -> i16:\n\
         \x20\x20\x20\x20let mut value: i16 = 0\n\
         \x20\x20\x20\x20while value < limit:\n\
         \x20\x20\x20\x20\x20\x20\x20\x20value = step(value)\n\
         \x20\x20\x20\x20return value\n",
    )
    .unwrap();
    assert!(json.contains("\"dialect\":\"modern\""));
    assert!(json.contains("\"op\":\"call\""));
    assert!(json.contains("\"kind\":\"branch\""));
    assert!(json.contains("\"storage\":\"local\""));
}

#[test]
fn rejects_assignment_to_let() {
    let error = compile_source(
        "fn bad() -> i16:\n\
         \x20\x20\x20\x20let value = 1\n\
         \x20\x20\x20\x20value = 2\n\
         \x20\x20\x20\x20return value\n",
    )
    .unwrap_err();
    assert!(error.message.contains("immutable"));
}

#[test]
fn rejects_non_boolean_condition() {
    let error = compile_source(
        "fn bad(value: i16) -> i16:\n\
         \x20\x20\x20\x20if value:\n\
         \x20\x20\x20\x20\x20\x20\x20\x20return 1\n\
         \x20\x20\x20\x20return 0\n",
    )
    .unwrap_err();
    assert!(error.message.contains("expected bool"));
}

#[test]
fn accepts_the_i16_minimum_literal() {
    let json = compile_source(
        "fn minimum() -> i16:\n\
         \x20\x20\x20\x20return -32768\n",
    )
    .unwrap();
    assert!(json.contains("\"value\":-32768"));
}

#[test]
fn integer_literals_are_checked_against_their_primitive_width() {
    let error = compile_source(
        "fn too_large() -> u8:\n\
         \x20\x20\x20\x20return 256\n",
    )
    .unwrap_err();
    assert!(error.message.contains("does not fit u8"));

    let error = compile_source(
        "fn negative() -> u32:\n\
         \x20\x20\x20\x20return -1\n",
    )
    .unwrap_err();
    assert!(error.message.contains("does not fit u32"));
}

#[test]
fn unsigned_and_float_operators_emit_distinct_hir_operations() {
    let json = compile_source(
        "fn quotient(a: u32, b: u32) -> u32:\n\
         \x20\x20\x20\x20return a // b\n\
         fn less(a: u16, b: u16) -> bool:\n\
         \x20\x20\x20\x20return a < b\n\
         fn product(a: f32, b: f32) -> f32:\n\
         \x20\x20\x20\x20return a * b\n",
    )
    .unwrap();
    assert!(json.contains("\"op\":\"udiv\""));
    assert!(json.contains("\"op\":\"below\""));
    assert!(json.contains("\"op\":\"fmul\""));
}

#[test]
fn arrays_and_f_strings_lower_to_structural_hir_and_streaming_calls() {
    let json = compile_source(
        "fn show(index: i16) -> void:\n\
         \x20\x20\x20\x20let mut values: i32[2] = [10, 20]\n\
         \x20\x20\x20\x20values[index] = values[index] + 1\n\
         \x20\x20\x20\x20print(f\"value={values[index]}\")\n",
    )
    .unwrap();
    assert!(json.contains("\"kind\":\"array\""));
    assert!(json.contains("\"tag\":\"array_element\""));
    for callee in [rt::PRINT_STRING, rt::PRINT_I4, rt::PRINT_NEWLINE] {
        assert!(json.contains(&format!("\"callee\":\"{callee}\"")), "{callee}");
    }
    assert!(json.contains("\"bytes\":[8,0,6,0,6,0,118,97,108,117,101,61,0]"));
}

#[test]
fn literal_array_bounds_are_checked_before_hir() {
    let error = compile_source(
        "fn bad() -> i32:\n\
         \x20\x20\x20\x20let values: i32[2] = [10, 20]\n\
         \x20\x20\x20\x20return values[2]\n",
    )
    .unwrap_err();
    assert!(error.message.contains("outside 0..2"));
}

/// matmul.mod spelled `[0, 0, ...]` three times: 192 stores that every unroll candidate carried.
#[test]
fn an_array_literal_of_one_repeated_value_fills_like_a_repeat_literal() {
    let spelled = compile_source(
        "fn zeros() -> i32:\n\
         \x20\x20\x20\x20let mut values: i32[8] = [0, 0, 0, 0, 0, 0, 0, 0]\n\
         \x20\x20\x20\x20return values[3]\n",
    )
    .unwrap();
    let repeated = compile_source(
        "fn zeros() -> i32:\n\
         \x20\x20\x20\x20let mut values: i32[8] = [0] * 8\n\
         \x20\x20\x20\x20return values[3]\n",
    )
    .unwrap();
    assert_eq!(spelled, repeated);
}

#[test]
fn an_array_literal_of_different_values_stores_each_element() {
    let json = compile_source(
        "fn pair() -> i32:\n\
         \x20\x20\x20\x20let mut values: i32[2] = [0, 1]\n\
         \x20\x20\x20\x20return values[1]\n",
    )
    .unwrap();
    assert!(!json.contains("$values_fill"));
}

#[test]
fn struct_array_for_loop_uses_projected_places_without_iterator_calls() {
    let json = compile_source(
        "struct pair:\n\
         \x20\x20\x20\x20mut left: i32\n\
         \x20\x20\x20\x20right: i32\n\
         fn bump() -> i32:\n\
         \x20\x20\x20\x20let mut pairs: pair[2] = [pair(left=1, right=2), pair(left=3, right=4)]\n\
         \x20\x20\x20\x20for item in &mut pairs:\n\
         \x20\x20\x20\x20\x20\x20\x20\x20item.left = item.left + 1\n\
         \x20\x20\x20\x20return pairs[1].left\n",
    )
    .unwrap();
    assert!(json.contains("\"kind\":\"opaque\",\"name\":\"pair\""));
    assert!(json.contains("\"name\":\"[pair; 2]\""));
    assert!(json.contains("\"tag\":\"projection\""));
    assert!(json.contains("\"op\":\"below\""));
    assert!(!json.contains("\"callee\":\"__iter"));
}

#[test]
fn range_loop_keeps_the_bound_type_and_lowers_without_a_runtime_iterator() {
    let json = compile_source(
        "fn sum(step_count: u16) -> u16:\n\
         \x20\x20\x20\x20let mut total: u16 = 0\n\
         \x20\x20\x20\x20for step_no in 0..step_count - 1:\n\
         \x20\x20\x20\x20\x20\x20\x20\x20total = total + step_no\n\
         \x20\x20\x20\x20return total\n",
    )
    .unwrap();
    let range_at = json.find("\"name\":\"$range_step_no\"").unwrap();
    let range_place = &json[range_at..range_at + json[range_at..].find('}').unwrap()];
    assert!(range_place.contains(&format!("\"type\":{}", type_id(TypeName::U16))));
    assert!(json.contains("\"op\":\"below\""));
    assert!(json.contains("\"op\":\"add\""));
    assert!(!json.contains("\"callee\":\"__iter"));
}

#[test]
fn range_loop_requires_integer_bounds() {
    let error = compile_source(
        "fn bad(limit: f32) -> void:\n\
         \x20\x20\x20\x20for item in 0..limit:\n\
         \x20\x20\x20\x20\x20\x20\x20\x20print(item)\n",
    )
    .unwrap_err();
    assert!(error.message.contains("range bounds must be integers"));
}

#[test]
fn fixed_point_literals_and_arithmetic_are_scaled_at_compile_time() {
    let json = compile_source(
        "type fixed8 = fixed i16, fraction=8\n\
         type fixed16 = fixed i32, fraction=16\n\
         fn calculate(a: fixed8, b: fixed8) -> fixed8:\n\
         \x20\x20\x20\x20let mut factor: fixed8 = 1.5\n\
         \x20\x20\x20\x20factor = 2.25\n\
         \x20\x20\x20\x20return 0.5 * a * factor / b\n",
    )
    .unwrap();
    assert!(json.contains("\"kind\":\"integer\",\"name\":\"fixed8\""));
    assert!(json.contains("\"kind\":\"integer\",\"name\":\"fixed16\""));
    assert!(json.contains(&format!("\"type\":{},\"value\":384", FIXED_START)));
    assert!(json.contains(&format!("\"type\":{},\"value\":576", FIXED_START)));
    for operation in ["convert", "mul", "sar", "shl", "div"] {
        assert!(json.contains(&format!("\"op\":\"{operation}\"")));
    }
}

#[test]
fn i32_fixed_arithmetic_stays_out_of_generic_i64_hir() {
    let json = compile_source(
        "type scalar = fixed i32, fraction=9\n\
         fn product(a: scalar, b: scalar) -> scalar:\n\
         \x20\x20\x20\x20return a * b\n\
         fn quotient(a: scalar, b: scalar) -> scalar:\n\
         \x20\x20\x20\x20return a / b\n",
    )
    .unwrap();

    assert!(json.contains("\"op\":\"fixed_mul\""));
    assert!(json.contains("\"op\":\"fixed_div\""));
    for operation in ["convert", "mul", "sar", "shl", "div"] {
        assert!(!json.contains(&format!("\"op\":\"{operation}\"")));
    }
}

#[test]
fn fixed_point_decimal_literals_round_once_and_must_fit_storage() {
    let rounded = compile_source(
        "type fixed8 = fixed i16, fraction=8\n\
         fn tenth() -> fixed8:\n\
         \x20\x20\x20\x20return 0.1\n",
    )
    .unwrap();
    assert!(rounded.contains(&format!("\"type\":{},\"value\":26", FIXED_START)));

    let too_large = compile_source(
        "type fixed8 = fixed i16, fraction=8\n\
         fn bad() -> fixed8:\n\
         \x20\x20\x20\x20return 128\n",
    )
    .unwrap_err();
    assert!(too_large.message.contains("does not fit"));
}

#[test]
fn separately_declared_fixed_point_types_do_not_mix_implicitly() {
    let error = compile_source(
        "type distance = fixed i16, fraction=8\n\
         type duration = fixed i16, fraction=8\n\
         fn bad(left: distance, right: duration) -> distance:\n\
         \x20\x20\x20\x20return left + right\n",
    )
    .unwrap_err();
    assert!(error.message.contains("distinct fixed-point types"));
}

#[test]
fn plain_for_view_is_immutable() {
    let error = compile_source(
        "struct item:\n\
         \x20\x20\x20\x20mut value: i16\n\
         fn bad() -> void:\n\
         \x20\x20\x20\x20let mut items: item[1] = [item(value=1)]\n\
         \x20\x20\x20\x20for one in &items:\n\
         \x20\x20\x20\x20\x20\x20\x20\x20one.value = 2\n",
    )
    .unwrap_err();
    assert!(error.message.contains("immutable"));
}

#[test]
fn local_structs_initialize_copy_and_update_through_projected_places() {
    let json = compile_source(
        "struct point:\n\
         \x20\x20\x20\x20x: i16\n\
         \x20\x20\x20\x20y: i16\n\
         struct body:\n\
         \x20\x20\x20\x20mut pos: point\n\
         \x20\x20\x20\x20mut mass: i16\n\
         fn move() -> i16:\n\
         \x20\x20\x20\x20let mut current: body = body(pos=point(x=1, y=2), mass=3)\n\
         \x20\x20\x20\x20let snapshot = current\n\
         \x20\x20\x20\x20current.pos = point(x=snapshot.pos.y, y=snapshot.pos.x)\n\
         \x20\x20\x20\x20current.mass += 4\n\
         \x20\x20\x20\x20return current.pos.x + current.pos.y + current.mass\n",
    )
    .unwrap();
    assert!(json.contains("\"name\":\"current\""));
    assert!(json.contains("\"name\":\"snapshot\""));
    assert!(json.matches("\"tag\":\"projection\"").count() >= 10);
    assert!(json.contains("\"op\":\"add\""));
}

#[test]
fn compound_assignment_evaluates_an_index_once() {
    let json = compile_source(
        "struct item:\n\
         \x20\x20\x20\x20mut value: i16\n\
         fn next() -> i16:\n\
         \x20\x20\x20\x20return 0\n\
         fn bump() -> i16:\n\
         \x20\x20\x20\x20let mut items: item[1] = [item(value=1)]\n\
         \x20\x20\x20\x20items[next()].value += 2\n\
         \x20\x20\x20\x20return items[0].value\n",
    )
    .unwrap();
    assert_eq!(json.matches("\"callee\":\"next\"").count(), 1);
}

#[test]
fn print_runtime_variants_use_short_byte_width_names() {
    let names: Vec<_> = print_builtins().into_iter().map(|(name, _)| name).collect();
    assert_eq!(
        names,
        [
            rt::PRINT_NEWLINE, rt::PRINT_STRING, rt::PRINT_BOOL, rt::PRINT_CHAR, rt::PRINT_I1, rt::PRINT_U1, rt::PRINT_I2, rt::PRINT_U2, rt::PRINT_I4, rt::PRINT_U4, rt::PRINT_R4,
            rt::PRINT_R8, rt::PRINT_Q2, rt::PRINT_Q4, rt::PRINT_VIEW,
        ]
    );
}
