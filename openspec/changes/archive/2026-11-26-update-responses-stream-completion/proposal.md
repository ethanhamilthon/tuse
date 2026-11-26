## Why
OpenAI Responses streams can deliver their terminal event before the HTTP body closes. Waiting for EOF adds avoidable latency after the model has finished.

## What Changes
- Finalize a Responses reply on `response.completed` or `response.incomplete`, preserving terminal metadata.
- Keep failed responses and premature EOF on their existing error paths.

## Impact
- Spec: `providers`.
- Implementation: OpenAI Responses SSE adapter and regression tests. No public schema changes.
