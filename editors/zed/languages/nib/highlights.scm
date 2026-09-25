; Later patterns take precedence.

(identifier) @variable

((identifier) @constant
  (#match? @constant "^_*[A-Z][A-Z0-9_]*$"))

(type_identifier) @type
(primitive_type) @type.builtin
((type_identifier) @type.builtin
  (#match? @type.builtin "^[iu][0-9]+$"))
((type_identifier) @type.builtin
  (#any-of? @type.builtin "vec" "dict" "iter" "Option" "Result" "Self"))

(field_identifier) @property
(register) @variable.special

(parameter name: (identifier) @variable.parameter)
(lambda_parameter name: (identifier) @variable.parameter)
(keyword_argument name: (identifier) @variable.parameter)
(type_parameter name: (type_identifier) @type)

((identifier) @variable.special
  (#any-of? @variable.special "self" "Self"))

; Declarations

(function_definition name: (identifier) @function)
(function_signature name: (identifier) @function)
(function_definition "." name: (identifier) @function.method)
(function_signature "." name: (identifier) @function.method)

(struct_declaration name: (type_identifier) @type)
(enum_declaration name: (type_identifier) @enum)
(protocol_declaration name: (type_identifier) @type.interface)
(type_declaration name: (type_identifier) @type)
(enum_variant name: (identifier) @variant)
(const_declaration name: (identifier) @constant)
(import_declaration alias: (identifier) @namespace)
(dotted_name (identifier) @namespace)

(attribute "@" @attribute name: (identifier) @attribute)

; Calls

(call_expression function: (identifier) @function)
(call_expression function: (field_expression field: (field_identifier) @function.method))
(generic_call function: (identifier) @function)
(generic_call function: (field_expression field: (field_identifier) @function.method))
(call_expression function: (primitive_type) @type.builtin)
((call_expression function: (identifier) @constructor)
  (#match? @constructor "^[A-Z]"))
((generic_call function: (identifier) @constructor)
  (#match? @constructor "^[A-Z]"))

(variant_expression name: (identifier) @variant)
(variant_pattern name: (identifier) @variant)
(struct_pattern type: (type_identifier) @constructor)

; Literals

(integer) @number
(float) @number
(true) @boolean
(false) @boolean
(char) @string.special
(string) @string
(fstring) @string
(escape_sequence) @string.escape
(format_specifier) @string.special
(asm_body) @embedded
(comment) @comment

(interpolation
  "{" @punctuation.special
  "}" @punctuation.special) @embedded

; Keywords

[
  "fn"
  "struct"
  "bits"
  "enum"
  "protocol"
  "type"
  "fixed"
  "fraction"
  "const"
  "var"
  "let"
  "mut"
  "import"
  "as"
  "extern"
  "asm"
  "unsafe"
  "with"
  "far"
  "near"
  "huge"
  "out"
  "clobbers"
] @keyword

(visibility) @keyword

[
  "if"
  "else"
  "match"
  "case"
  "while"
  "loop"
  "for"
  "in"
  "break"
  "continue"
  "return"
  "yield"
] @keyword.control

[
  "is"
  "not"
] @keyword.operator

; Operators and punctuation

[
  "+" "-" "*" "/" "//" "%"
  "&" "|" "^" "~" "<<" ">>"
  "==" "!=" "<" "<=" ">" ">="
  "&&" "||" "!"
  "=" "+=" "-=" "*=" "/=" "//=" "%=" "&=" "|=" "^=" "<<=" ">>="
  "->" ".." "?"
] @operator

["(" ")" "[" "]" "{" "}"] @punctuation.bracket
["," "." ":"] @punctuation.delimiter
