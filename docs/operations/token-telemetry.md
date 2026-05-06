# Token Telemetry

Use this runbook when checking token usage in Run Detail or Mission Control.

## Source Of Truth

Token usage comes only from real provider events. For this slice, OpenAI Responses `response.completed` events from the Platform API and ChatGPT backend are the supported source.

Taugentic records a durable `tokenUsageRecorded` event with:

- prompt tokens
- completion tokens
- cached tokens when provided
- reasoning tokens when provided
- provider
- model

No heuristic or estimated token counts are used.

## Operator Checks

1. Open Run Detail for a completed or active native OpenAI run.
2. In the Membrane view, check the Outputs section for prompt, completion, cached, reasoning, and total token fields.
3. Open Mission Control and check Token Usage for aggregate prompt, completion, cached, and reasoning totals in the diagnostics window.
4. If fields are unknown, confirm the provider emitted `usage` in `response.completed`.

## Expected Event

The durable event shape is:

```json
{
  "tokenUsageRecorded": {
    "runId": "run-...",
    "promptTokens": "11000",
    "completionTokens": "1345",
    "cachedTokens": "2000",
    "reasoningTokens": "345",
    "model": "gpt-...",
    "provider": "openai",
    "recordedAtMs": "..."
  }
}
```

All uint64 fields cross the desktop boundary as `bigint` after validation.

## Non-Goals

This telemetry does not calculate cost. Pricing tables, cost budgets, and provider-specific non-OpenAI usage adapters are separate Phase 3 follow-ups.
