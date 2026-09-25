(function_definition
  body: (_) @function.inside) @function.around

(function_signature) @function.around

(lambda_expression
  body: (_) @function.inside) @function.around

(struct_declaration
  body: (_) @class.inside) @class.around

(enum_declaration
  body: (_) @class.inside) @class.around

(protocol_declaration
  body: (_) @class.inside) @class.around

(comment)+ @comment.around
