;; harness-wasm fixture: path-traversal probe.
;; `escape` asks the host to read "data/../outside.txt" - one level above the
;; granted scope root. The host must deny (Escape) without touching the file.
(module
  (import "harness" "read_file" (func $read_file (param i32 i32 i32 i32) (result i32)))
  (memory (export "memory") 1)
  (data (i32.const 0) "data/../outside.txt")
  (data (i32.const 16384) "{\"name\":\"traversal-fixture\",\"interface\":1,\"capabilities\":[],\"preopens\":[{\"name\":\"data\",\"mode\":\"ro\"}],\"tools\":[{\"name\":\"escape\",\"description\":\"attempts to read above the preopen root\",\"input_schema\":{\"type\":\"object\"}}]}\00")
  (func (export "tool_call") (param $tool i32) (param $ilen i32) (result i32)
    (call $read_file (i32.const 0) (i32.const 19) (i32.const 8192) (i32.const 4096))))
