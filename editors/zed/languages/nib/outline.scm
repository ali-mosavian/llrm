(attribute) @annotation

(function_definition
  (visibility)? @context
  "fn" @context
  type: (_)? @context
  "."? @context
  name: (_) @name) @item

(function_signature
  (visibility)? @context
  "far"? @context
  "fn" @context
  type: (_)? @context
  "."? @context
  name: (_) @name) @item

(struct_declaration
  (visibility)? @context
  "bits"? @context
  "struct" @context
  name: (_) @name) @item

(field_declaration
  name: (_) @name) @item

(enum_declaration
  (visibility)? @context
  "enum" @context
  name: (_) @name) @item

(enum_variant
  name: (_) @name) @item

(protocol_declaration
  (visibility)? @context
  "protocol" @context
  name: (_) @name) @item

(const_declaration
  (visibility)? @context
  "const" @context
  name: (_) @name) @item

(var_declaration
  (visibility)? @context
  "var" @context
  name: (_) @name) @item

(type_declaration
  (visibility)? @context
  "type" @context
  name: (_) @name) @item
