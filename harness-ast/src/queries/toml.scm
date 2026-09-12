; toml tags: table headers and array-of-table headers are sections, pair
; keys are variables. Keys are bare_key/quoted_key/dotted_key (the _key
; rule is hidden in tree-sitter-toml-ng); direct-child patterns keep
; table headers from matching pair keys nested inside.
(table (bare_key) @name.definition.section) @_span
(table (quoted_key) @name.definition.section) @_span
(table (dotted_key) @name.definition.section) @_span
(table_array_element (bare_key) @name.definition.section) @_span
(table_array_element (quoted_key) @name.definition.section) @_span
(table_array_element (dotted_key) @name.definition.section) @_span
(pair (bare_key) @name.definition.variable) @_span
(pair (quoted_key) @name.definition.variable) @_span
(pair (dotted_key) @name.definition.variable) @_span
