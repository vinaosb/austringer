; rust tags: definitions and references (tree-sitter-rust node names)
(function_item name: (identifier) @name.definition.function) @_span
(function_signature_item name: (identifier) @name.definition.function) @_span
(struct_item name: (type_identifier) @name.definition.struct) @_span
(enum_item name: (type_identifier) @name.definition.enum) @_span
(enum_variant name: (identifier) @name.definition.enumMember) @_span
(trait_item name: (type_identifier) @name.definition.interface) @_span
(mod_item name: (identifier) @name.definition.module) @_span
(const_item name: (identifier) @name.definition.constant) @_span
(static_item name: (identifier) @name.definition.constant) @_span
(type_item name: (type_identifier) @name.definition.type) @_span
(macro_definition name: (identifier) @name.definition.macro) @_span
(call_expression function: (identifier) @name.reference.call)
(call_expression function: (field_expression field: (field_identifier) @name.reference.call))
(call_expression function: (scoped_identifier name: (identifier) @name.reference.call))
(macro_invocation macro: (identifier) @name.reference.call)
