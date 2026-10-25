(function_definition declarator: (function_declarator declarator: (identifier) @name.definition.function)) @_span
(struct_specifier name: (type_identifier) @name.definition.struct) @_span
(enum_specifier name: (type_identifier) @name.definition.enum) @_span
(type_definition declarator: (type_identifier) @name.definition.type) @_span
(call_expression function: (identifier) @name.reference.call)