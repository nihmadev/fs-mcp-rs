# Benchmark methodology

Rust and Node.js servers must run on the same machine, filesystem and power profile.
Each implementation receives identical JSON-RPC requests over HTTP without a proxy.

## Required measurements

1. Cold start to successful `initialize`.
2. Idle working set after 60 seconds.
3. `ping` latency: p50, p95 and maximum.
4. Listing directories with 1K, 10K and 100K entries.
5. Reading 4 KiB, 1 MiB and 64 MiB ranges.
6. Literal and regex search over the same source tree.
7. Throughput with 1, 8 and 32 concurrent clients.
8. Cancellation latency for long searches.
9. CPU time and peak working set during every scenario.
10. A 30-minute soak test with repeated mixed operations.

## Fairness rules

- Use release builds and warm up each server before latency tests.
- Report cold and warm filesystem-cache runs separately.
- Keep response limits and search semantics identical.
- Validate result equality before accepting performance numbers.
- Run at least five rounds and publish every raw result.
- Do not combine protocol, startup and filesystem timings into one number.

## Output

Raw JSON belongs in `benchmark-results/`. The final report must include hardware,
Windows version, Rust version, Node.js version, commit hashes and configuration.