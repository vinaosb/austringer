;; harness-wasm fixture: memory-hog probe.
;; `hog` grows linear memory 1 MiB at a time until the host ResourceLimiter
;; refuses (expected: MemoryCapExceeded typed error via the grow-denied trap).
(module
  (memory (export "memory") 1)
  (data (i32.const 16384) "{\"name\":\"memory-hog-fixture\",\"interface\":1,\"capabilities\":[],\"preopens\":[],\"tools\":[{\"name\":\"hog\",\"description\":\"grows linear memory until the limiter refuses\",\"input_schema\":{\"type\":\"object\"}}]}\00")
  (func (export "tool_call") (param $tool i32) (param $ilen i32) (result i32)
    (local $r i32)
    (loop $grow
      (local.set $r (memory.grow (i32.const 16)))
      (if (i32.eq (local.get $r) (i32.const -1))
        (then (unreachable)))
      (br $grow))
    (unreachable)))
