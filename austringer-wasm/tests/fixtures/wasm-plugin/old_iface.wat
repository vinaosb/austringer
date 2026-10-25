;; austringer-wasm fixture: plugin written against interface 0 - older than the
;; minimum the host supports. Load must fail with InterfaceTooOld guidance.
(module
  (memory (export "memory") 1)
  (data (i32.const 16384) "{\"name\":\"old-iface-fixture\",\"interface\":0,\"capabilities\":[],\"preopens\":[],\"tools\":[{\"name\":\"noop\",\"description\":\"ancient interface build\",\"input_schema\":{\"type\":\"object\"}}]}\00")
  (func (export "tool_call") (param $tool i32) (param $ilen i32) (result i32)
    i32.const 0))
