# e^e is not an assistant.

It is a deterministic decision runtime.

Built in Rust, it operates as a closed-loop system:
belief state → strategy selection → execution → evaluation → memory update.

Key properties:
- no hidden state
- no uncontrolled randomness (bounded stochasticity)
- explicit risk modeling
- horizon-based decision evaluation
- adaptive strategy weighting

The system is designed to evolve under constraints,
not to generate responses.

Target: autonomous orchestration layer for complex systems.

---

# eva_runtime_with_task_validator demo

Компактная демонстрационная версия EVA для локального запуска.

## Сценарии

1. `cargo run` — список доступных команд.
2. `cargo run -- --once` — локальный runtime cycle с русским фазовым отчётом.
3. `cargo run -- --repo <REPO_URL>` — анализ репозитория и patch report по файлам.
4. `cargo run -- --serve` — HTTP runtime daemon с OpenAI-compatible локальной моделью.
5. `cargo run --bin github_repo_discover`, `github_repo_prepare`, `benchmark_batch` — benchmark pipeline.

## Быстрый старт

### Локальный runtime

```powershell
Copy-Item .\input.example.json .\input.json
cargo run -- --once
```

### Repo patch mode

```powershell
cargo run -- --repo <REPO_URL>
```

Результат:
- `./eva_output/report.md`
- `./eva_output/summary.json`

### HTTP runtime daemon

```powershell
cargo run -- --serve
```

Если рядом есть `eva.runtime.json`, daemon загрузит его автоматически. В текущем локальном конфиге default model — `qwen3-local` через Ollama на `127.0.0.1:11434`, а `eva-lite` остается автономным fallback без интернета, без загрузки LLM в память и без внешних model server процессов. Явный путь:

```powershell
cargo run -- --serve --config eva.runtime.example.json
```

Несколько локальных OpenAI-compatible серверов/моделей подключаются только явно и только после появления реального локального файла модели:

```powershell
cargo run -- --serve `
  --model-file fast=/models/tiny.gguf `
  --model-file deep=/models/large.gguf `
  --model-endpoint fast=tiny-model@http://127.0.0.1:1234/v1/chat/completions `
  --model-endpoint deep=large-model@http://127.0.0.1:8080/v1/chat/completions
```

Запуск внешнего model server вместе с EVA имеет смысл только когда указанный `.gguf`/`.safetensors` файл уже существует на диске:

```powershell
cargo run -- --serve `
  --model-file fast=/models/tiny.gguf `
  --start-server "fast=llama-server -m /models/tiny.gguf --port 1234" `
  --model-endpoint fast=tiny-model@http://127.0.0.1:1234/v1/chat/completions
```

Переменные окружения:
- `EVA_LISTEN_ADDR` — адрес daemon, по умолчанию `127.0.0.1:8765`
- `EVA_MODEL_URL` — OpenAI-compatible `/v1/chat/completions`, по умолчанию `http://127.0.0.1:1234/v1/chat/completions`
- `EVA_MODEL` — имя локальной модели
- `EVA_MODEL_FILE` — локальный файл модели для `--model-url`/`--model`
- `EVA_MODEL_ENDPOINTS` — список `ID=MODEL@URL`, разделитель `;`
- `EVA_MODEL_FILES` — список `ID=/path/to/model.gguf`, разделитель `;`
- `EVA_MODEL_SERVER_COMMANDS` — список `ID=COMMAND`, разделитель `;`
- `EVA_MODEL_API_KEY` — optional bearer token

Endpoints:
- `GET /health`
- `GET /models`
- `POST /runtime/cycle` с JSON `{"goal":"...","context":"..."}`
- `POST /model/chat` с JSON `{"prompt":"...","model_id":"eva-lite"}`

Локальный model path использует минимальный HTTP/1.1 клиент на `std::net` и поддерживает только `http://` endpoints. Online GitHub discovery вынесен за feature:

Без внешней модели daemon использует встроенный `eva-lite` backend. Он не грузит LLM в память, не ходит в интернет и возвращает короткие детерминированные рекомендации. Если для внешнего backend указан `local_model_path` или `--model-file ID=PATH`, EVA не будет обращаться к этому backend, пока файл модели не найден на диске.

```powershell
cargo run --features github-online --bin github_repo_discover
```

### Offline benchmark demo

```powershell
cargo run --bin github_repo_discover -- --fixture fixtures/github_search_fixture.json
cargo run --bin github_repo_prepare
cargo run --bin benchmark_batch
```

Результат:
- `benchmarks/rust_cases.json`
- `benchmarks/rust_cases_prepared.json`
- `benchmarks/rust_cases_ready.json`
- `benchmarks/rust_batch_report.json`

## Git bootstrap

Будущий origin:
- `https://github.com/peccatos/eva-brain-repo`
# EVA Operator Runtime

Read-only operator dashboard:

```bash
cargo run -- tui
```

Current runtime validation:

```bash
cargo run -- status
```

Phase 15.1H to 15.7X plus the 16.0P readiness scaffold focuses on visibility, truthful metrics, candidate queue hygiene, governed release-candidate metadata, and the honest green gate. See:

- `docs/runtime_tui.md`
- `docs/metrics_semantics.md`
- `docs/candidate_queue.md`
- `docs/release_candidate_flow.md`
- `docs/phase_15x_status.md`
- `docs/phase_16_evolution_core_readiness.md`

Current compressed path:

- `15.1H` TUI hydration truth
- `15.2R` truthful metrics and queue display
- `15.6X` first governed release-candidate path
- `15.7X` green gate attempt
- `16.0P` readiness scaffold only, without evolution-core activation

## EVE Production Agent v1

Local governed software engineering loop:

```bash
cargo run -- task "document production agent v1"
cargo run -- inspect
cargo run -- plan <TASK_ID>
cargo run -- propose <TASK_ID>
cargo run -- approve <PROPOSAL_ID>
cargo run -- apply <PROPOSAL_ID>
cargo run -- validate
cargo run -- report <TASK_ID>
cargo run -- pr-summary <TASK_ID>
cargo run -- agent-readiness
cargo run -- tui
```

The loop is operator-gated. Proposals do not apply themselves, OpenAI output cannot bypass approval, and PR summaries do not push or create remote PRs.

Docs:

- `docs/production_agent_v1.md`
- `docs/openai_llm_adapter.md`
- `docs/agent_task_contract.md`
- `docs/agent_workspace_inspector.md`
- `docs/agent_proposal_apply.md`
- `docs/agent_validation_report.md`
- `docs/agent_pr_summary.md`
- `docs/agent_safety_model.md`

## EVE Production Agent v2 and Learning

Phase 17P to 20G extends the local agent with proposal preview, dry-run, repo
mapping, outcome memory, pattern extraction, work fitness, strategy selection,
and governed self-improvement proposals:

```bash
cargo run -- repo-map
cargo run -- proposal-show <PROPOSAL_ID>
cargo run -- apply --dry-run <PROPOSAL_ID>
cargo run -- task-outcomes
cargo run -- outcome-analyze
cargo run -- patterns
cargo run -- strategy-memory
cargo run -- fitness
cargo run -- strategy-select "document agent behavior"
cargo run -- self-improve propose
cargo run -- agent-v2-readiness
```

Future evolution is based on real task outcomes: tasks, plans, proposals,
approvals, apply results, validations, reports, PR summaries, operator
decisions, failed proposals, and successful patches.

Additional docs:

- `docs/production_agent_v2.md`
- `docs/openai_structured_proposals.md`
- `docs/proposal_preview_dry_run.md`
- `docs/code_edit_engine_v1.md`
- `docs/repo_aware_planner.md`
- `docs/task_outcome_memory.md`
- `docs/agent_v2_readiness.md`

## EVE Fix

```bash
cargo run -- fix .
cargo run -- fix . --apply
cargo run -- fix . --only ci
cargo run -- fix . --only tests
```

`fix` is a safe facade command over EVE's governed repair pipeline. It detects one actionable issue, proposes a minimal patch, stores evidence, and only mutates files with `--apply`.

See also:

- `docs/phase_21_eve_fix.md`
- `docs/phase_18_outcome_learning.md`
- `docs/phase_19_work_fitness.md`
- `docs/phase_20_governed_self_improvement.md`
