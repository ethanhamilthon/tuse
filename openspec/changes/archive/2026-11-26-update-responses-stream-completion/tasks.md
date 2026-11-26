## Tasks

- [x] 1. Stop the Responses SSE reader at a successful or incomplete terminal event.
- [x] 2. Verify both terminal events with an open HTTP body and run the adapter tests.
- [x] 3. Re-measure Responses TUI overhead against the same upstream flow.

## Verification

- Delayed-EOF PTY run (30 turns, 400 ms between terminal event and EOF): terminal event to completed frame p95 4.06 ms; total TUI completion overhead vs direct SSE p95 24.81 ms.
- Live Responses via a local forwarding relay (GPT-6.2 Sol, 30/30 valid turns): TUI overhead p95 46.39 ms to first visible text marker and 48.27 ms to completion, including the relay. Maximum observed completion overhead was 133.97 ms.
- `cargo test --workspace --locked` passed.
