(class_declaration name: (identifier) @name.definition.class) @_span
(interface_declaration name: (identifier) @name.definition.interface) @_span
(struct_declaration name: (identifier) @name.definition.struct) @_span
(enum_declaration name: (identifier) @name.definition.enum) @_span
(method_declaration name: (identifier) @name.definition.method) @_span
(property_declaration name: (identifier) @name.definition.property) @_span
(namespace_declaration name: (identifier) @name.definition.module) @_span
(invocation_expression function: (identifier) @name.reference.call)
(invocation_expression function: (member_access_expression name: (identifier) @name.reference.call))