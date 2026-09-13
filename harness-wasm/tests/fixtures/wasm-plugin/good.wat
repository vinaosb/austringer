;; harness-wasm fixture: well-behaved echo plugin, interface 1.
;; ABI v1 layout (see harness-wasm crate docs):
;;   [0, 4096)      guest scratch
;;   [4096, 8192)   host-written tool input  (cap 4096)
;;   [8192, 12288)  guest-written tool output (cap 4096)
;;   [16384, 24576) NUL-terminated UTF-8 JSON manifest (cap 8192)
(module
  (import "harness" "log" (func $log (param i32 i32 i32)))
  (memory (export "memory") 1)
  (data (i32.const 16384) "{\"name\":\"echo-fixture\",\"interface\":1,\"capabilities\":[],\"preopens\":[{\"name\":\"data\",\"mode\":\"ro\"}],\"tools\":[{\"name\":\"echo\",\"description\":\"echoes the tool input verbatim\",\"input_schema\":{\"type\":\"object\"}}]}\00")
  (func (export "tool_call") (param $tool i32) (param $ilen i32) (result i32)
    (local $i i32)
    ;; host-visible log line carrying the raw input (untrusted text path)
    (call $log (i32.const 0) (i32.const 4096) (local.get $ilen))
    ;; copy input region to output region byte for byte, return length
    (loop $copy
      (if (i32.ge_u (local.get $i) (local.get $ilen))
        (then (return (local.get $ilen))))
      (i32.store8
        (i32.add (i32.const 8192) (local.get $i))
        (i32.load8_u (i32.add (i32.const 4096) (local.get $i))))
      (local.set $i (i32.add (local.get $i) (i32.const 1)))
      (br $copy))
    (unreachable)))
