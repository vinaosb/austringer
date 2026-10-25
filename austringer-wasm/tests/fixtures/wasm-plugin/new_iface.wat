;; austringer-wasm fixture: plugin written against interface 3 - newer than the
;; host's maximum (interface 2, the M6 media-adapter seam). Load must fail
;; with InterfaceTooNew guidance.
(module
  (memory (export "memory") 1)
  (data (i32.const 16384) "{\"name\":\"new-iface-fixture\",\"interface\":3,\"capabilities\":[],\"preopens\":[],\"tools\":[{\"name\":\"noop\",\"description\":\"future interface build\",\"input_schema\":{\"type\":\"object\"}}]}\00")
  (func (export "tool_call") (param $tool i32) (param $ilen i32) (result i32)
    i32.const 0))
