;; harness-wasm fixture: plugin written against interface 2 - newer than the
;; host's maximum. Load must fail with InterfaceTooNew guidance.
(module
  (memory (export "memory") 1)
  (data (i32.const 16384) "{\"name\":\"new-iface-fixture\",\"interface\":2,\"capabilities\":[],\"preopens\":[],\"tools\":[{\"name\":\"noop\",\"description\":\"future interface build\",\"input_schema\":{\"type\":\"object\"}}]}\00")
  (func (export "tool_call") (param $tool i32) (param $ilen i32) (result i32)
    i32.const 0))
