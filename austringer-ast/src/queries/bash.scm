; bash tags: functions and variable assignments, command calls
; (tree-sitter-bash node names)
(function_definition name: (word) @name.definition.function) @_span
(variable_assignment name: (variable_name) @name.definition.variable) @_span
(command_name . (word) @name.reference.call)
