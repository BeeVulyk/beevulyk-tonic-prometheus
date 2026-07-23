# beevulyk-tonic-prometheus

Prometheus metrics middleware for [tonic](https://github.com/hyperium/tonic) gRPC
servers. Exposes a Tower `Layer` (`MetricsLayer`) that instruments each RPC with:

- `grpc_server_started_total` / `grpc_server_handled_total` counters (by service,
  method, and gRPC status code).
- `grpc_server_handling_seconds` histogram (RPC duration).
- Legacy `function_calls_*` counter/histogram/gauge broken out by HTTP method
  and path (kept for backward compatibility).

The `grpc_metrics_disabled` cargo feature turns the layer into a no-op.

Call `metrics::encode_to_string()` to render the current registry in the
Prometheus text format (e.g. from a `/metrics` HTTP endpoint).

## Origin

Forked from [`ITYFT/yft-service-sdk`](https://github.com/ITYFT/yft-service-sdk)
at tag `0.1.16`, sub-crate `yft-tonic-prometheus`. Extracted here as a
standalone crate under the BeeVulyk org and versioned from `0.1.0`.
