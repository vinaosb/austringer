(call
  target: (identifier) @_def_kw
  (arguments (call target: (identifier) @name.definition.function))
  (#eq? @_def_kw "def"))
(call
  target: (identifier) @_defp_kw
  (arguments (call target: (identifier) @name.definition.function))
  (#eq? @_defp_kw "defp"))
(call
  target: (identifier) @_module_kw
  (arguments (alias) @name.definition.module)
  (#eq? @_module_kw "defmodule"))