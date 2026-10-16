# Flip 🪙

Flip is a parody decision model: every question is answered by a coin flip, behind an API that is 1:1 wire-compatible with TypeSafe's Jev (`POST /v1/systemone`) and OpenRouter's Decisions (`POST /api/alpha/decisions`). Every decision model is a coin flip. Flip admits it.

## Quickstart

Prerequisites: Rust **1.82+** and Node **20.19+** or **22.12+**. Node **24** is used in development.

From the repo root:

```sh
npm ci --prefix frontend
npm run build --prefix frontend
cargo run
```

Live demo: <https://flip.lavx.hu>.

Open <http://localhost:8080>. The landing page includes a docs-style API tester and a coin playground. Bring a question; the coin brings the methodology.

The backend is a Rust axum application, crate `flip`, in `Cargo.toml` and `src/`, with tests in `tests/`. The React 19 + Vite SPA lives in `frontend/`; its build output, `frontend/dist`, is embedded into the binary via `rust-embed` at release build time.

| Environment variable | Default | Purpose |
| --- | --- | --- |
| `FLIP_PORT` | `8080` | HTTP listen port. |
| `FLIP_BIND` | `127.0.0.1` | HTTP bind address. |
| `FLIP_ALLOW_TEST_SEED` | `false` | Enable the reproducible-answer test hook. |

## Docker

```sh
docker build -t flip:local .
docker run --rm -p 127.0.0.1:8080:8080 flip:local
# Or: docker compose up --build -d
```

The multi-stage image builds the frontend, embeds it into the Rust binary, and runs as a non-root user. It exposes port 8080 and checks `/health`. The container needs no database, credentials, or persistent volumes.

`compose.deploy.yaml` is the deployment configuration for `flip.lavx.hu`, using an existing `traefik_net` network and `myresolver` TLS resolver. Set `FLIP_IMAGE` to the exact release image and run `docker compose -f compose.deploy.yaml up -d`. The application port is not published directly by this configuration.

## The API

Both `POST /v1/systemone` and `POST /api/alpha/decisions` share one handler and the same successful response shape. Send a bearer token, a model, a state, and named questions:

```sh
curl -X POST http://localhost:8080/v1/systemone \
  -H 'Authorization: Bearer banana' \
  -H 'Content-Type: application/json' \
  --data '{"model":"flip-latest","state":"The team has one last meeting on Friday.","questions":{"ship_it":{"type":"noul","instructions":"Should we ship before the weekend?"}}}'
```

One possible response:

```json
{
  "model": "flip-1.0.0",
  "answers": {
    "ship_it": { "type": "noul", "noul": 1.0 }
  },
  "usage": {
    "input_tokens": 42,
    "output_tokens": 1,
    "cost": 0.0
  }
}
```

The response always reports `model: "flip-1.0.0"`, regardless of the requested alias. Token usage is synthetic; the coin charges `0.0`.

Each request example below is a single entry in `questions`; each answer is the corresponding entry in `answers`:

| Type | Request question | Example answer |
| --- | --- | --- |
| `noul` | `{"type":"noul","instructions":"Ship it?"}` | `{"type":"noul","noul":1.0}` |
| `choice` | `{"type":"choice","criteria":{"ship":"Release now","wait":"Wait until Monday"}}` | `{"type":"choice","choice":"ship","probabilities":{"ship":1.0,"wait":0.0},"confidence":1.0}` |
| `score` | `{"type":"score","criteria":["Low","Medium","High"]}` | `{"type":"score","score":2.0,"legend":{"0":"Low","1":"Medium","2":"High"},"probabilities":{"0":0.0,"1":0.0,"2":1.0},"confidence":1.0}` |
| `box` | `{"type":"box","instructions":"Where is the coin?"}` | `{"type":"box","box":{"top":0.1,"bottom":0.8,"left":0.2,"right":0.9},"confidence":1.0}` |

A `noul` returns `0.0` or `1.0`, or `0.5` on an edge landing; it has no separate `confidence` field. Scores use zero-based level indices. Box coordinates are normalized to the image dimensions.

## Confidence

Flip implements TypeSafe's published formulas exactly:

| Question type | Confidence |
| --- | --- |
| `choice` | `(p_max - 1/n) / (1 - 1/n)` |
| `score` | `max(0, 1 - spread / MAD_unif)` |

Here, `n` is the number of choices or score levels, and `p_max` is the highest choice probability. For scores, `spread` is the probability-weighted absolute distance from the mode, using the lowest index to break ties; `MAD_unif` is the mean absolute deviation of uniformly distributed levels from their midpoint.

A normal Choice or Score flip has confidence **1.0**. Edge landings have confidence **0.0** for multi-option Choice and Score; a single option remains certain at **1.0**. Noul has no confidence field, and Box reports **0.0** on an edge landing. Probabilities are full-precision. Confidence measures the distribution, not the coin's wisdom: accuracy is exactly random chance, **1/n** on n-way choices and **50%** on binary nouls.

## Model aliases

| Model | Behavior |
| --- | --- |
| `flip-latest` | The stable coin. |
| `flip-preview` | Resolves to `flip-1.1.0`, the two-sided coin. |
| `flip-edge` | Forces edge landings: noul `0.5`, uniform Choice/Score probabilities, full-image Box. Single-option Choice/Score remain certain. |
| `flip-overloaded` | Returns HTTP **529** so you can test retry logic. The coin itself is fine. |

Any other model id is accepted and resolves to `flip-1.0.0`. `GET /v1/models` lists the aliases; use the same bearer header. Even `flip-preview` keeps `flip-1.0.0` in the response's `model` field.

## The edge case

There is a **1-in-6000** chance per question that the coin lands on its edge. Choice and Score probabilities become uniform. Confidence is **0.0** for multi-option Choice/Score and Box; a single-option Choice/Score has confidence **1.0**. A noul becomes `0.5`, a score lands at the midpoint **`(n-1)/2`**, between levels when that midpoint is fractional, and a box becomes the whole image: `{"top":0.0,"bottom":1.0,"left":0.0,"right":1.0}`.

Use `flip-edge` to summon this rare display of intellectual honesty on demand.

## Errors

| HTTP status | When it happens |
| --- | --- |
| **401** | Missing bearer token or an empty token. Any non-empty token is accepted; `banana` works. |
| **413** | Request body over **32 MiB**, on the wire or after decoding. |
| **415** | Unsupported or double `Content-Encoding`. Supported encodings: `gzip`, `x-gzip`, and `zstd`. |
| **400** | Malformed JSON, a BOM, non-finite numbers, nesting depth over **64** in `state`, question `instructions`, or question `criteria`, or corrupt compression. |
| **422** | Field validation fails. |
| **429** | Never. The coin has no rate-limit mood swings. |
| **529** | The requested model is `flip-overloaded`. |

On the TypeSafe path, `/v1/systemone`, error codes are snake_case strings:

```json
{"error":{"code":"missing_bearer","message":"Authorization must contain Bearer followed by a non-empty token; we accept banana."}}
```

On the OpenRouter path, `/api/alpha/decisions`, the code is numeric and the envelope includes a UUID `id`:

```json
{"error":{"code":401,"message":"Authorization must contain Bearer followed by a non-empty token; we accept banana."},"id":"af324d0a-5d70-4e03-9b25-5fc68cf70690"}
```

## Compatibility notes

Flip replicates the wire shapes and confidence math. Its deliberate differences are part of the joke:

- Authentication is dummy auth, and arbitrary model ids are accepted.
- Images are ignored. The coin doesn't look at your photos.
- Multipart envelopes are accepted, but file bytes are discarded.
- There are no context limits, subject to the request size and nesting limits above.
- Answers are independent. Apply your own consistency checks when combining decisions.

## Development

Run the backend checks from the repo root:

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --locked
```

Run the frontend checks from `frontend/`:

```sh
npm run typecheck
npm run build
```

For frontend development, keep the backend running with `cargo run`, then start Vite from the repo root:

```sh
npm run dev --prefix frontend
```

The Vite dev server proxies `/v1` and `/api` to `http://localhost:8080`.

For reproducible answers, start the backend with the test hook enabled:

```sh
FLIP_ALLOW_TEST_SEED=true cargo run
```

Then add `X-Flip-Seed: <u64>` to a request. The header makes answers reproducible when `FLIP_ALLOW_TEST_SEED=true`; the default is `false`.

## Credits / License

Vendored LMSUI components come from the **arc-library** registry under the **MIT** license, at commit **`884de9e`**. Per-file SHA-256 provenance is recorded in [frontend/vendor/lmsui/PROVENANCE.md](frontend/vendor/lmsui/PROVENANCE.md).

Flip is distributed under the [MIT license](LICENSE). Fonts retain their bundled SIL Open Font License; museum coin images are public domain (CC0), with credits in the frontend.

The coin is fair; we checked.
