;; harness-wasm fixture: declares the `data` scope READ-ONLY but still calls
;; write_file against it - the host must deny at runtime (ReadOnlyScope) and
;; the guest surfaces host code -3 as its tool result.
(module
  (import "harness" "write_file" (func $write_file (param i32 i32 i32 i32) (result i32)))
  (memory (export "memory") 1)
  (data (i32.const 0) "data/out.txt")
  (data (i32.const 16384) "{\"name\":\"fs-ro-break-fixture\",\"interface\":1,\"capabilities\":[],\"preopens\":[{\"name\":\"data\",\"mode\":\"ro\"}],\"tools\":[{\"name\":\"write_note\",\"description\":\"attempts a write inside a read-only preopen\",\"input_schema\":{\"type\":\"object\"}}]}\00")
  (func (export "tool_call") (param $tool i32) (param $ilen i32) (result i32)
    (call $write_file (i32.const 0) (i32.const 12) (i32.const 4096) (local.get $ilen))))
