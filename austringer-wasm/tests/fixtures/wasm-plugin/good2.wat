;; austringer-wasm fixture: echo plugin v2 (hot-reload marker).
;; Identical to good.wat except the echoed output gains a "-v2" suffix,
;; so a swapped plugin is distinguishable from its predecessor.
(module
  (import "austringer" "log" (func $log (param i32 i32 i32)))
  (memory (export "memory") 1)
  (data (i32.const 16384) "{\"name\":\"echo-fixture\",\"interface\":1,\"capabilities\":[],\"preopens\":[{\"name\":\"data\",\"mode\":\"ro\"}],\"tools\":[{\"name\":\"echo\",\"description\":\"echoes the tool input and appends -v2\",\"input_schema\":{\"type\":\"object\"}}]}\00")
  (func (export "tool_call") (param $tool i32) (param $ilen i32) (result i32)
    (local $i i32)
    (call $log (i32.const 0) (i32.const 4096) (local.get $ilen))
    (loop $copy
      (if (i32.ge_u (local.get $i) (local.get $ilen))
        (then
          (i32.store8 (i32.add (i32.const 8192) (local.get $ilen)) (i32.const 45))
          (i32.store8 (i32.add (i32.add (i32.const 8192) (local.get $ilen)) (i32.const 1)) (i32.const 118))
          (i32.store8 (i32.add (i32.add (i32.const 8192) (local.get $ilen)) (i32.const 2)) (i32.const 50))
          (return (i32.add (local.get $ilen) (i32.const 3)))))
      (i32.store8
        (i32.add (i32.const 8192) (local.get $i))
        (i32.load8_u (i32.add (i32.const 4096) (local.get $i))))
      (local.set $i (i32.add (local.get $i) (i32.const 1)))
      (br $copy))
    (unreachable)))
