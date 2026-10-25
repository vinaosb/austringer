(function_declaration name: (identifier) @name.definition.function) @_span
(generator_function_declaration name: (identifier) @name.definition.function) @_span
(class_declaration name: (type_identifier) @name.definition.class) @_span
(abstract_class_declaration name: (type_identifier) @name.definition.class) @_span
(method_definition name: (property_identifier) @name.definition.method) @_span
(variable_declarator name: (identifier) @name.definition.variable) @_span
(interface_declaration name: (type_identifier) @name.definition.interface) @_span
(type_alias_declaration name: (type_identifier) @name.definition.type) @_span
(enum_declaration name: (identifier) @name.definition.enum) @_span
(function_signature name: (identifier) @name.definition.function) @_span
(call_expression function: (identifier) @name.reference.call)
(call_expression function: (member_expression property: (property_identifier) @name.reference.call))