export type Primitive = "noul" | "choice" | "score" | "box";
export type Preset = Primitive | "mixed";

type JsonValue = string | number | boolean | null | JsonValue[] | { [key: string]: JsonValue };

export interface DecisionQuestion {
  type: Primitive;
  instructions: string;
  criteria?: JsonValue;
}

export interface DecisionRequest {
  model: string;
  state: string | Record<string, JsonValue> | JsonValue[];
  questions: Record<string, DecisionQuestion>;
}

export const primitiveTypes: Primitive[] = ["noul", "choice", "score", "box"];
export const defaultState = "Help! My payouts have been failing for 3 days.";

export const questionIds: Record<Primitive, string> = {
  noul: "is_urgent",
  choice: "department",
  score: "frustration",
  box: "mug",
};

export function createRequest(
  preset: Preset,
  model = "flip-latest",
  state = defaultState,
): DecisionRequest {
  const questions: Record<Primitive, DecisionQuestion> = {
    noul: {
      type: "noul",
      instructions: "Does this convey urgency?",
      criteria: { true: "Urgent", false: "Not urgent" },
    },
    choice: {
      type: "choice",
      instructions: "Which department should handle this?",
      criteria: {
        billing: "Payments and payouts",
        support: "Technical issues",
        sales: "New business",
      },
    },
    score: {
      type: "score",
      instructions: "How frustrated is the customer?",
      criteria: ["Calm", "Frustrated", "Very frustrated"],
    },
    box: {
      type: "box",
      instructions: "Locate the mug in the image.",
      criteria: {},
    },
  };

  return {
    model,
    state,
    questions: Object.fromEntries(
      (preset === "mixed" ? primitiveTypes : [preset]).map((type) => [
        questionIds[type],
        questions[type],
      ]),
    ),
  };
}

export function pretty(value: unknown): string {
  return JSON.stringify(value, null, 2) ?? "null";
}

function exampleUsage(type: Primitive): string {
  const inputTokens = Math.ceil(
    new TextEncoder().encode(pretty(primitiveRequest(type))).length / 4,
  );
  return JSON.stringify({ input_tokens: inputTokens, output_tokens: 1, cost: 0 });
}

export const exampleResponses: Record<Primitive, string> = {
  noul: `{
  "model": "flip-1.0.0",
  "answers": {
    "is_urgent": { "type": "noul", "noul": 1.0 }
  },
  "usage": ${exampleUsage("noul")}
}`,
  choice: `{
  "model": "flip-1.0.0",
  "answers": {
    "department": {
      "type": "choice",
      "choice": "billing",
      "probabilities": { "billing": 1.0, "support": 0.0, "sales": 0.0 },
      "confidence": 1.0
    }
  },
  "usage": ${exampleUsage("choice")}
}`,
  score: `{
  "model": "flip-1.0.0",
  "answers": {
    "frustration": {
      "type": "score",
      "score": 2,
      "legend": { "0": "Calm", "1": "Frustrated", "2": "Very frustrated" },
      "probabilities": { "0": 0.0, "1": 0.0, "2": 1.0 },
      "confidence": 1.0
    }
  },
  "usage": ${exampleUsage("score")}
}`,
  box: `{
  "model": "flip-1.0.0",
  "answers": {
    "mug": {
      "type": "box",
      "box": { "top": 0.2, "bottom": 0.8, "left": 0.3, "right": 0.6 },
      "confidence": 1.0
    }
  },
  "usage": ${exampleUsage("box")}
}`,
};

export function primitiveRequest(type: Primitive): DecisionRequest {
  const request = createRequest(type);
  if (type === "box") {
    request.state = { image: "https://example.com/mug.jpg" };
  }
  return request;
}

const quickstartOrigin = typeof window === "undefined" ? "http://localhost:8080" : window.location.origin;

export const quickstartCurl = `curl -X POST ${quickstartOrigin}/v1/systemone \\
  -H 'Authorization: Bearer banana' \\
  -H 'Content-Type: application/json' \\
  --data '${pretty(createRequest("noul"))}'`;

const quickstartPayload = pretty(createRequest("noul"));
const indentExample = (source: string, spaces: number) => source.replace(/\n/g, `\n${" ".repeat(spaces)}`);

export const quickstartExamples = [
  { id: "curl", label: "cURL", language: "bash", filename: "Terminal", setup: "Run in your terminal. No SDK required.", code: quickstartCurl },
  {
    id: "javascript", label: "JavaScript", language: "javascript", filename: "flip.mjs",
    setup: "Uses built-in fetch. Run with node flip.mjs.",
    code: `const response = await fetch("${quickstartOrigin}/v1/systemone", {
  method: "POST",
  headers: {
    "Authorization": "Bearer banana",
    "Content-Type": "application/json",
  },
  body: JSON.stringify(${indentExample(quickstartPayload, 2)}),
});

if (!response.ok) {
  throw new Error(\`HTTP \${response.status}: \${await response.text()}\`);
}
console.log(await response.json());`,
  },
  {
    id: "python", label: "Python", language: "python", filename: "flip.py",
    setup: "Python 3 standard library. Run with python3 flip.py.",
    code: `import json
from urllib.request import Request, urlopen

payload = ${quickstartPayload}

request = Request(
    "${quickstartOrigin}/v1/systemone",
    data=json.dumps(payload).encode("utf-8"),
    headers={
        "Authorization": "Bearer banana",
        "Content-Type": "application/json",
    },
    method="POST",
)
with urlopen(request, timeout=20) as response:
    print(json.load(response))`,
  },
  {
    id: "rust", label: "Rust", language: "rust", filename: "src/main.rs",
    setup: "Add dependencies: cargo add reqwest --features blocking,json && cargo add serde_json. Run with cargo run.",
    code: `use reqwest::blocking::Client;
use serde_json::{json, Value};
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let payload = json!(${indentExample(quickstartPayload, 4)});
    let response: Value = Client::builder()
        .timeout(Duration::from_secs(20))
        .build()?
        .post("${quickstartOrigin}/v1/systemone")
        .bearer_auth("banana")
        .json(&payload)
        .send()?
        .error_for_status()?
        .json()?;

    println!("{}", serde_json::to_string_pretty(&response)?);
    Ok(())
}`,
  },
];
