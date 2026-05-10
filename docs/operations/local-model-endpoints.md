# Local Model Endpoints

Taugentic supports local and open-source model servers through the native
OpenAI-compatible chat-completions path. Local HTTP endpoints remain in the
`NativeLoop` harness; ACP is only for external provider processes that speak ACP.

## Runtime Presets

| Preset | Default Base URL | Chat | Models | Tools | Responses | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| Ollama | `http://127.0.0.1:11434/v1` | `/v1/chat/completions` | `/v1/models` | Supported, model-dependent | Supported | API keys may be dummy values for OpenAI SDK compatibility. |
| LM Studio | `http://127.0.0.1:1234/v1` | `/v1/chat/completions` | `/v1/models` | Supported, model-dependent | Supported | Tool quality depends on native/default tool support for the loaded model. |
| llama.cpp server | `http://127.0.0.1:8080/v1` | `/v1/chat/completions` | `/v1/models` | Supported with server/template caveats | Supported | API-key auth is optional; tool use may require `--jinja` and compatible templates. |
| vLLM | `http://127.0.0.1:8000/v1` | `/v1/chat/completions` | `/v1/models` | Supported with launch flags | Supported | Auto tool calling requires parser/template configuration. |
| TGI | `http://127.0.0.1:3000/v1` | `/v1/chat/completions` | Deployment-dependent | Not supported by current Messages API guidance | Not primary | Requires TGI 1.4+ and a model with a chat template. |
| Custom OpenAI-compatible | User supplied | `/v1/chat/completions` | `/v1/models` when available | Probe required | Probe required | Use this for OpenRouter-compatible or self-hosted compatible servers. |

## CLI

List runtime profiles:

```sh
ta agent-runtime list
```

Configure the custom local profile:

```sh
ta agent-runtime local add \
  --base-url http://127.0.0.1:11434/v1 \
  --standard ollama-openai \
  --model gpt-oss:20b
```

Probe reachability and model discovery:

```sh
ta agent-runtime local test \
  --base-url http://127.0.0.1:11434/v1 \
  --standard ollama-openai \
  --model gpt-oss:20b
```

Probe tool-call compatibility:

```sh
ta agent-runtime local test \
  --base-url http://127.0.0.1:11434/v1 \
  --standard ollama-openai \
  --model gpt-oss:20b \
  --tool-call
```

Set a local profile model:

```sh
ta agent-runtime local set-model --profile runtime-local-ollama gpt-oss:20b
```

Clear local endpoint config from a profile:

```sh
ta agent-runtime local remove --profile runtime-local-custom
```

## Capability Rules

Reachability is not the same as agent readiness. A server can support plain chat
while failing Taugentic's tool loop. Use the tool-call probe for agent workflows
and treat `toolsUnsupported` as a signal to pick a model/server configuration
with function-calling support.

Secrets should be provided through environment references when possible:

```sh
ta agent-runtime local add \
  --base-url http://127.0.0.1:8000/v1 \
  --standard vllm-openai \
  --auth-mode bearer-env \
  --api-key-env VLLM_API_KEY \
  --model NousResearch/Meta-Llama-3-8B-Instruct
```
