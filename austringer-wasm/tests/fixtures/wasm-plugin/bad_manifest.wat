;; austringer-wasm fixture: manifest region carries malformed JSON.
;; Load must fail with the typed Manifest error (not a panic, not a trap).
(module
  (memory (export "memory") 1)
  (data (i32.const 16384) "{oops\00")
  (func (export "tool_call") (param $tool i32) (param $ilen i32) (result i32)
    i32.const 0))
