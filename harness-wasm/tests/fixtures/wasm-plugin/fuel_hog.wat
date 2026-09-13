;; harness-wasm fixture: fuel-exhaustion probe.
;; `spin` enters an unbounded loop; the host's per-call fuel budget is the
;; only thing that terminates it (expected: FuelExhausted typed error).
(module
  (memory (export "memory") 1)
  (data (i32.const 16384) "{\"name\":\"fuel-hog-fixture\",\"interface\":1,\"capabilities\":[],\"preopens\":[],\"tools\":[{\"name\":\"spin\",\"description\":\"spins forever until fuel runs out\",\"input_schema\":{\"type\":\"object\"}}]}\00")
  (func (export "tool_call") (param $tool i32) (param $ilen i32) (result i32)
    (loop $spin (br $spin))
    (unreachable)))
