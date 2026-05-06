# ta-auth-openai

Native OpenAI ChatGPT subscription OAuth support for Taugentic.

This crate owns the browser PKCE flow, the short-lived local callback server,
token exchange contracts, token-claim parsing, and browser-launch fallback
surface. It also owns the Taugentic credential store for persisted ChatGPT
subscription credentials.