(class_declaration name: (identifier) @name.definition.class) @_span
(interface_declaration name: (identifier) @name.definition.interface) @_span
(enum_declaration name: (identifier) @name.definition.enum) @_span
(method_declaration name: (identifier) @name.definition.method) @_span
(field_declaration (variable_declarator name: (identifier) @name.definition.property)) @_span
(method_invocation name: (identifier) @name.reference.call)