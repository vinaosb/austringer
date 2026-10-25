(function_declaration name: (identifier) @name.definition.function) @_span
(generator_function_declaration name: (identifier) @name.definition.function) @_span
(class_declaration name: (identifier) @name.definition.class) @_span
(method_definition name: (property_identifier) @name.definition.method) @_span
(variable_declarator name: (identifier) @name.definition.variable) @_span
(call_expression function: (identifier) @name.reference.call)
(call_expression function: (member_expression property: (property_identifier) @name.reference.call))