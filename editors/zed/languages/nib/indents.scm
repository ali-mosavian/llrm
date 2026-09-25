(_ "[" "]" @end) @indent
(_ "{" "}" @end) @indent
(_ "(" ")" @end) @indent

(function_definition) @start.def
(struct_declaration) @start.struct
(enum_declaration) @start.enum
(protocol_declaration) @start.protocol
(if_statement) @start.if
(else_clause) @start.else
(while_statement) @start.while
(loop_statement) @start.loop
(for_statement) @start.for
(match_statement) @start.match
(match_arm) @start.case
(with_statement) @start.with
(unsafe_statement) @start.unsafe
