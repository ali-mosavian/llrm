/**
 * @file Tree-sitter grammar for Nib, llrm's modern language (docs/language-spec.md).
 * @license MIT
 */

/// <reference types="tree-sitter-cli/dsl" />
// @ts-check

// Spec section 3, loosest first. The ternary binds tighter than `!`.
const PREC = {
  lambda: -1,
  or: 1,
  and: 2,
  not: 3,
  ternary: 4,
  compare: 5,
  bit_or: 6,
  bit_xor: 7,
  bit_and: 8,
  shift: 9,
  add: 10,
  multiply: 11,
  unary: 12,
  postfix: 13,
};

const PRIMITIVES = [
  'char', 'i8', 'u8', 'i16', 'u16', 'i32', 'u32', 'f32', 'f64',
  'string', 'addr', 'bool', 'void',
];

const ASSIGNMENT_OPERATORS = [
  '=', '+=', '-=', '*=', '/=', '//=', '%=', '&=', '|=', '^=', '<<=', '>>=',
];

const commaSep1 = (rule) => seq(rule, repeat(seq(',', rule)));
const commaSep = (rule) => optional(commaSep1(rule));

// `:`, then an indented block.
const suite = ($, name) => seq(':', $._newline, $._indent, field(name, $.block));

module.exports = grammar({
  name: 'nib',

  word: $ => $.identifier,

  extras: $ => [/\s/, $.comment],

  externals: $ => [
    $._newline,
    $._indent,
    $._dedent,
    $.asm_body,
    $._try_question,
    $._ternary_question,
    $._error_sentinel,
    ')',
    ']',
    '}',
  ],

  supertypes: $ => [$._expression, $._pattern, $._type, $._statement],

  conflicts: $ => [
    [$._expression, $._type_name],
    [$._expression, $.scoped_type_identifier],
    [$._simple_type, $._expression],
  ],

  rules: {
    source_file: $ => repeat($._item),

    _item: $ => choice(
      $.import_declaration,
      $.function_definition,
      $.struct_declaration,
      $.enum_declaration,
      $.protocol_declaration,
      $.const_declaration,
      $.var_declaration,
      $.type_declaration,
      $.extern_block,
      $.export_block,
    ),

    comment: _ => token(seq('#', /.*/)),

    // Declarations

    attribute: $ => seq('@', field('name', $.identifier), optional(field('arguments', $.arguments)), $._newline),

    visibility: _ => 'pub',

    import_declaration: $ => seq(
      'import',
      field('module', $.dotted_name),
      optional(seq('as', field('alias', $.identifier))),
      $._newline,
    ),

    dotted_name: $ => seq($.identifier, repeat(seq('.', $.identifier))),

    function_definition: $ => seq(
      repeat($.attribute),
      optional($.visibility),
      $._function_header,
      suite($, 'body'),
    ),

    function_signature: $ => seq(
      repeat($.attribute),
      optional($.visibility),
      optional('far'),
      $._function_header,
      $._newline,
    ),

    _function_header: $ => seq(
      'fn',
      optional(seq(field('type', choice($.primitive_type, alias($.identifier, $.type_identifier))), '.')),
      field('name', $.identifier),
      optional(field('type_parameters', $.type_parameters)),
      field('parameters', $.parameters),
      optional(seq('->', field('return_type', $._type))),
    ),

    type_parameters: $ => seq('[', commaSep1($.type_parameter), optional(','), ']'),

    type_parameter: $ => seq(
      field('name', alias($.identifier, $.type_identifier)),
      optional(seq(':', field('bound', $._type))),
    ),

    parameters: $ => seq('(', commaSep($.parameter), optional(','), ')'),

    parameter: $ => seq(
      field('name', $.identifier),
      ':',
      field('type', $._type),
      optional(seq('=', field('default', $._expression))),
    ),

    struct_declaration: $ => seq(
      repeat($.attribute),
      optional($.visibility),
      optional('bits'),
      'struct',
      field('name', alias($.identifier, $.type_identifier)),
      optional(field('type_parameters', $.type_parameters)),
      ':',
      optional(field('backing', $._type)),
      $._newline,
      $._indent,
      field('body', $.field_list),
    ),

    field_list: $ => seq(repeat1($.field_declaration), $._dedent),

    field_declaration: $ => seq(
      optional('mut'),
      field('name', alias($.identifier, $.field_identifier)),
      ':',
      field('type', $._type),
      $._newline,
    ),

    enum_declaration: $ => seq(
      repeat($.attribute),
      optional($.visibility),
      'enum',
      field('name', alias($.identifier, $.type_identifier)),
      optional(field('type_parameters', $.type_parameters)),
      ':',
      optional(field('backing', $._type)),
      $._newline,
      $._indent,
      field('body', $.variant_list),
    ),

    variant_list: $ => seq(repeat1($.enum_variant), $._dedent),

    enum_variant: $ => seq(
      field('name', $.identifier),
      optional(seq('(', commaSep1($.variant_field), optional(','), ')')),
      optional(seq('=', field('tag', $.integer))),
      $._newline,
    ),

    variant_field: $ => seq(
      optional(seq(field('name', alias($.identifier, $.field_identifier)), ':')),
      field('type', $._type),
    ),

    protocol_declaration: $ => seq(
      repeat($.attribute),
      optional($.visibility),
      'protocol',
      field('name', alias($.identifier, $.type_identifier)),
      optional(field('type_parameters', $.type_parameters)),
      ':',
      $._newline,
      $._indent,
      field('body', $.declaration_list),
    ),

    declaration_list: $ => seq(repeat1($.function_signature), $._dedent),

    const_declaration: $ => seq(
      repeat($.attribute),
      optional($.visibility),
      optional('let'),
      'const',
      field('name', $.identifier),
      optional(seq(':', field('type', $._type))),
      '=',
      field('value', $._expression),
      $._newline,
    ),

    var_declaration: $ => seq(
      repeat($.attribute),
      optional($.visibility),
      'var',
      field('name', $.identifier),
      optional(seq(':', field('type', $._type))),
      '=',
      field('value', $._expression),
      $._newline,
    ),

    // `type fixed8 = fixed i16, fraction=8`
    type_declaration: $ => seq(
      repeat($.attribute),
      optional($.visibility),
      'type',
      field('name', alias($.identifier, $.type_identifier)),
      '=',
      field('value', $.fixed_type),
      $._newline,
    ),

    fixed_type: $ => seq(
      'fixed',
      field('storage', $.primitive_type),
      ',',
      'fraction',
      '=',
      field('fraction', $.integer),
    ),

    extern_block: $ => seq(
      'extern',
      field('abi', $.string),
      ':',
      $._newline,
      $._indent,
      field('body', $.extern_list),
    ),

    extern_list: $ => seq(repeat1($.function_signature), $._dedent),

    export_block: $ => seq(
      'export',
      field('abi', $.string),
      ':',
      $._newline,
      $._indent,
      field('body', $.export_list),
    ),

    export_list: $ => seq(repeat1($.function_definition), $._dedent),

    // Statements

    block: $ => seq(repeat1($._statement), $._dedent),

    _statement: $ => choice(
      $.let_statement,
      $.const_declaration,
      $.function_definition,
      $.expression_statement,
      $.assignment_statement,
      $.return_statement,
      $.yield_statement,
      $.break_statement,
      $.continue_statement,
      $.if_statement,
      $.while_statement,
      $.loop_statement,
      $.for_statement,
      $.match_statement,
      $.with_statement,
      $.unsafe_statement,
      $.asm_statement,
    ),

    let_statement: $ => seq(
      'let',
      optional('mut'),
      field('pattern', $._pattern),
      optional(seq(':', field('type', $._type))),
      '=',
      field('value', $._expression),
      choice($._newline, seq('else', suite($, 'alternative'))),
    ),

    expression_statement: $ => seq($._expression, $._newline),

    assignment_statement: $ => seq(
      field('left', $._expression),
      field('operator', choice(...ASSIGNMENT_OPERATORS)),
      field('right', $._expression),
      $._newline,
    ),

    return_statement: $ => seq('return', optional($._expression), $._newline),

    yield_statement: $ => seq('yield', $._expression, $._newline),

    break_statement: $ => seq('break', $._newline),

    continue_statement: $ => seq('continue', $._newline),

    if_statement: $ => seq(
      'if',
      field('condition', $._expression),
      suite($, 'consequence'),
      optional(field('alternative', $.else_clause)),
    ),

    else_clause: $ => seq('else', choice($.if_statement, suite($, 'body'))),

    while_statement: $ => seq('while', field('condition', $._expression), suite($, 'body')),

    loop_statement: $ => seq('loop', suite($, 'body')),

    for_statement: $ => seq($._for_head, suite($, 'body')),

    _for_head: $ => seq(
      'for',
      optional('case'),
      field('pattern', $._pattern),
      'in',
      field('iterable', choice($._expression, $.range)),
    ),

    range: $ => seq(field('start', $._expression), '..', field('end', $._expression)),

    match_statement: $ => seq(
      'match',
      field('subject', $._expression),
      ':',
      $._newline,
      $._indent,
      field('body', $.match_block),
    ),

    match_block: $ => seq(repeat1($.match_arm), $._dedent),

    match_arm: $ => seq(field('pattern', $._pattern), suite($, 'body')),

    with_statement: $ => seq(
      'with',
      optional('mut'),
      field('name', $.identifier),
      '=',
      field('value', $._expression),
      suite($, 'body'),
    ),

    unsafe_statement: $ => seq('unsafe', suite($, 'body')),

    // `asm(ah=0, out=(cx=let high), clobbers=[al, flags]):` and its raw lines.
    asm_statement: $ => seq(
      'asm',
      '(',
      commaSep(choice($.asm_input, $.asm_outputs, $.asm_clobbers)),
      optional(','),
      ')',
      ':',
      $.asm_body,
    ),

    asm_input: $ => seq(field('register', alias($.identifier, $.register)), '=', field('value', $._expression)),

    asm_outputs: $ => seq('out', '=', '(', commaSep($.asm_output), optional(','), ')'),

    asm_output: $ => seq(
      field('register', alias($.identifier, $.register)),
      '=',
      field('target', choice(seq('let', optional('mut'), $.identifier), $._expression)),
    ),

    asm_clobbers: $ => seq('clobbers', '=', '[', commaSep(alias($.identifier, $.register)), optional(','), ']'),

    // Patterns (spec section 6)

    _pattern: $ => choice(
      $.identifier,
      $._literal_pattern,
      $.tuple_pattern,
      $.struct_pattern,
      $.variant_pattern,
      $.sequence_pattern,
    ),

    _literal_pattern: $ => choice(
      $.integer,
      $.float,
      $.char,
      $.string,
      $.true,
      $.false,
      $.negative_literal,
    ),

    negative_literal: $ => seq('-', choice($.integer, $.float)),

    tuple_pattern: $ => seq('(', commaSep($._pattern), optional(','), ')'),

    struct_pattern: $ => seq(
      field('type', alias($.identifier, $.type_identifier)),
      '(',
      commaSep($._pattern),
      optional(','),
      ')',
    ),

    // `.some(x)`, `Shape.circle(c, r)`, `module.Enum.variant`
    variant_pattern: $ => seq(
      choice('.', repeat1(seq(alias($.identifier, $.type_identifier), '.'))),
      field('name', $.identifier),
      optional(seq('(', commaSep($._pattern), optional(','), ')')),
    ),

    sequence_pattern: $ => seq('[', commaSep(choice($._pattern, $.rest_pattern)), optional(','), ']'),

    rest_pattern: $ => seq('*', $.identifier),

    // Types

    _type: $ => choice(
      $._simple_type,
      $.tuple_type,
      $.generic_type,
      $.array_type,
    ),

    _simple_type: $ => choice(
      $.primitive_type,
      $._type_name,
      $.pointer_type,
      $.reference_type,
      $.function_type,
      $.extern_function_type,
    ),

    primitive_type: _ => choice(...PRIMITIVES),

    _type_name: $ => choice(alias($.identifier, $.type_identifier), $.scoped_type_identifier),

    scoped_type_identifier: $ => seq(
      field('path', $.identifier),
      repeat(seq('.', $.identifier)),
      '.',
      field('name', alias($.identifier, $.type_identifier)),
    ),

    // `vec[T]`, `Result[T, E]`, `Name[T, 2]`
    generic_type: $ => prec(3, seq(field('type', $._type_name), field('type_arguments', $.type_arguments))),

    type_arguments: $ => seq('[', commaSep1(choice($._type, $.integer)), optional(','), ']'),

    // `u8[4]`, `i16[N, M]`
    array_type: $ => prec(1, seq(
      field('element', choice(
        $.primitive_type,
        $.pointer_type,
        $.reference_type,
        $.tuple_type,
        $.generic_type,
        $.array_type,
      )),
      '[',
      commaSep1(field('length', $._dimension)),
      ']',
    )),

    _dimension: $ => choice($.integer, $.identifier, alias($.scoped_type_identifier, $.scoped_identifier)),

    tuple_type: $ => seq('(', commaSep1($._type), optional(','), ')'),

    // `*far mut T`
    pointer_type: $ => prec(2, seq(
      '*',
      field('distance', choice('near', 'far', 'huge')),
      optional('mut'),
      field('target', $._simple_type),
    )),

    // `&T`, `&mut T`, `&T[N]`, and views `&[T]`, `&[T, 2]`
    reference_type: $ => prec.right(seq(
      '&',
      optional('mut'),
      field('target', choice($._type, $.view_type)),
    )),

    view_type: $ => seq('[', field('element', $._type), optional(seq(',', field('rank', $._dimension))), ']'),

    function_type: $ => prec.right(seq(
      'fn',
      '(',
      commaSep($._type),
      optional(','),
      ')',
      optional(seq('->', field('return_type', $._type))),
    )),

    extern_function_type: $ => seq('extern', field('abi', $.string), $.function_type),

    // Expressions (spec section 3)

    _expression: $ => choice(
      $.identifier,
      $.primitive_type,
      $.integer,
      $.float,
      $.char,
      $.string,
      $.fstring,
      $.true,
      $.false,
      $.variant_expression,
      $.parenthesized_expression,
      $.tuple_expression,
      $.array_expression,
      $.dict_expression,
      $.comprehension,
      $.dict_comprehension,
      $.generator_expression,
      $.unary_expression,
      $.borrow_expression,
      $.not_expression,
      $.binary_expression,
      $.conditional_expression,
      $.try_expression,
      $.call_expression,
      $.generic_call,
      $.index_expression,
      $.slice_expression,
      $.field_expression,
      $.lambda_expression,
    ),

    variant_expression: $ => seq('.', field('name', $.identifier)),

    parenthesized_expression: $ => seq('(', $._expression, ')'),

    tuple_expression: $ => seq('(', $._expression, ',', commaSep($._expression), optional(','), ')'),

    array_expression: $ => seq('[', commaSep($._expression), optional(','), ']'),

    dict_expression: $ => seq('{', commaSep($.pair), optional(','), '}'),

    pair: $ => seq(field('key', $._expression), ':', field('value', $._expression)),

    comprehension: $ => seq('[', field('body', $._expression), $._comprehension_clauses, ']'),

    dict_comprehension: $ => seq('{', field('body', $.pair), $._comprehension_clauses, '}'),

    generator_expression: $ => seq('(', field('body', $._expression), $._comprehension_clauses, ')'),

    _comprehension_clauses: $ => seq($.for_clause, repeat(choice($.for_clause, $.if_clause))),

    for_clause: $ => $._for_head,

    if_clause: $ => seq('if', $._expression),

    unary_expression: $ => prec(PREC.unary, seq(
      field('operator', choice('-', '~', '*')),
      field('operand', $._expression),
    )),

    borrow_expression: $ => prec(PREC.unary, seq(
      '&',
      optional('mut'),
      field('operand', $._expression),
    )),

    not_expression: $ => prec(PREC.not, seq('!', field('operand', $._expression))),

    binary_expression: $ => {
      const table = [
        [PREC.or, '||'],
        [PREC.and, '&&'],
        [PREC.compare, choice('==', '!=', '<', '<=', '>', '>=', 'is', seq('is', 'not'))],
        [PREC.bit_or, '|'],
        [PREC.bit_xor, '^'],
        [PREC.bit_and, '&'],
        [PREC.shift, choice('<<', '>>')],
        [PREC.add, choice('+', '-')],
        [PREC.multiply, choice('*', '/', '//', '%')],
      ];
      return choice(...table.map(([precedence, operator]) => prec.left(precedence, seq(
        field('left', $._expression),
        field('operator', operator),
        field('right', $._expression),
      ))));
    },

    // `cond ? a : b`; the scanner tells this `?` from a propagating one.
    conditional_expression: $ => prec.right(PREC.ternary, seq(
      field('condition', $._expression),
      alias($._ternary_question, '?'),
      field('consequence', $._expression),
      ':',
      field('alternative', $._expression),
    )),

    try_expression: $ => prec(PREC.postfix, seq($._expression, alias($._try_question, '?'))),

    call_expression: $ => prec(PREC.postfix, seq(
      field('function', $._expression),
      field('arguments', $.arguments),
    )),

    // `Pair[u8, bool](first=7)`, `block.cast[u16]()`
    generic_call: $ => prec.dynamic(1, prec(PREC.postfix, seq(
      field('function', $._expression),
      field('type_arguments', alias($._call_type_arguments, $.type_arguments)),
      field('arguments', $.arguments),
    ))),

    _call_type_arguments: $ => seq('[', commaSep1($._type), optional(','), ']'),

    arguments: $ => seq('(', commaSep(choice($._expression, $.keyword_argument)), optional(','), ')'),

    keyword_argument: $ => seq(field('name', $.identifier), '=', field('value', $._expression)),

    index_expression: $ => prec(PREC.postfix, seq(
      field('value', $._expression),
      '[',
      commaSep1(field('index', $._expression)),
      optional(','),
      ']',
    )),

    slice_expression: $ => prec(PREC.postfix, seq(
      field('value', $._expression),
      '[',
      optional(field('start', $._expression)),
      ':',
      optional(field('end', $._expression)),
      ']',
    )),

    field_expression: $ => prec(PREC.postfix, seq(
      field('value', $._expression),
      '.',
      field('field', alias($.identifier, $.field_identifier)),
    )),

    lambda_expression: $ => prec.right(PREC.lambda, seq(
      field('parameters', choice(alias('||', $.lambda_parameters), $.lambda_parameters)),
      field('body', $._expression),
    )),

    lambda_parameters: $ => seq('|', commaSep($.lambda_parameter), '|'),

    lambda_parameter: $ => seq(field('name', $.identifier), optional(seq(':', field('type', $._type)))),

    // Literals

    true: _ => 'true',
    false: _ => 'false',

    integer: _ => token(choice(
      /0[xX][0-9a-fA-F_]+/,
      /0[bB][01_]+/,
      /0[oO][0-7_]+/,
      /[0-9]+/,
    )),

    float: _ => token(choice(
      /[0-9]+\.[0-9]+([eE][+-]?[0-9]+)?/,
      /[0-9]+[eE][+-]?[0-9]+/,
      /\.[0-9]+([eE][+-]?[0-9]+)?/,
    )),

    char: _ => token(seq("'", choice(/[^'\\\n]/, /\\x[0-9a-fA-F]{2}/, /\\[^\n]/), "'")),

    escape_sequence: _ => token.immediate(choice(/\\x[0-9a-fA-F]{2}/, /\\[0nrt\\"{}]/)),

    string: $ => seq(
      '"',
      repeat(choice(alias(token.immediate(prec(1, /[^"\\\n]+/)), $.string_content), $.escape_sequence)),
      token.immediate('"'),
    ),

    fstring: $ => seq(
      'f"',
      repeat(choice(
        alias(token.immediate(prec(1, /[^"\\{}\n]+/)), $.string_content),
        $.escape_sequence,
        alias(token.immediate(choice('{{', '}}')), $.escape_sequence),
        $.interpolation,
      )),
      token.immediate('"'),
    ),

    interpolation: $ => seq(
      token.immediate('{'),
      field('value', $._expression),
      optional(seq(':', field('format', $.format_specifier))),
      '}',
    ),

    format_specifier: _ => token.immediate(/[^}"\n]+/),

    identifier: _ => /[a-zA-Z_][a-zA-Z0-9_]*/,
  },
});
