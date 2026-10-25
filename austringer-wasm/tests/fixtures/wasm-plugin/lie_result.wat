;; austringer-wasm fixture: lying length probe.
;; `liar` claims a 9999-byte result without producing it; the host must reject
;; the oversized claim (OutputTooLarge) instead of trusting the guest count.
(module
  (memory (export "memory") 1)
  (data (i32.const 16384) "{\"name\":\"liar-fixture\",\"interface\":1,\"capabilities\":[],\"preopens\":[],\"tools\":[{\"name\":\"liar\",\"description\":\"claims an oversized result length\",\"input_schema\":{\"type\":\"object\"}}]}\00")
  (func (export "tool_call") (param $tool i32) (param $ilen i32) (result i32)
    i32.const 9999))
