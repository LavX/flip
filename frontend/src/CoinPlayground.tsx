import { useEffect, useRef, useState } from "react";
import hadrian from "./assets/images/hadrian.jpg";
import {
  createRequest,
  defaultState,
  primitiveTypes,
  questionIds,
  type Primitive,
} from "./data";
import { Alert } from "@/registry/components/alert/alert";
import { Badge } from "@/registry/components/badge/badge";
import { Button } from "@/registry/components/button/button";
import SectionPanel from "./SectionPanel";
import { CodeBlock } from "@/registry/components/code-block/code-block";
import { Input } from "@/registry/components/input/input";
import SegmentedControl, {
  type Segment,
} from "@/registry/components/segmented-control/segmented-control";
import { requestJson, type JsonRequestResult } from "./api";

type DecisionModel = "flip-latest" | "flip-edge";

interface RunSnapshot {
  preset: Primitive;
  model: DecisionModel;
  state: string;
  instructions: string;
}

interface CompletedRun {
  snapshot: RunSnapshot;
  result: JsonRequestResult;
}

interface NoulAnswer {
  type: "noul";
  noul: 0 | 0.5 | 1;
}

interface ChoiceAnswer {
  type: "choice";
  choice: string;
  probabilities: Record<string, number>;
  confidence: number;
}

interface ScoreAnswer {
  type: "score";
  score: number;
  legend: Record<string, string>;
  probabilities: Record<string, number>;
  confidence: number;
}

interface BoxAnswer {
  type: "box";
  box: { top: number; bottom: number; left: number; right: number };
  confidence: number;
}

type DecisionAnswer = NoulAnswer | ChoiceAnswer | ScoreAnswer | BoxAnswer;

type CoinFace = "obverse" | "reverse" | "edge";

const COIN_SPIN_MS = 620;
const COIN_LANDING_MS = 620;

interface ParsedDecision {
  model: string;
  answer: DecisionAnswer;
  error?: string;
}

const primitiveOptions: Segment[] = primitiveTypes.map((value) => ({
  value,
  label: ({ noul: "Yes / no", choice: "Choice", score: "Score", box: "Box" })[value],
}));

const modelOptions: Segment[] = [
  { value: "flip-latest", label: "fair" },
  { value: "flip-edge", label: "on edge" },
];

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function isFiniteNumber(value: unknown): value is number {
  return typeof value === "number" && Number.isFinite(value);
}

function isProbability(value: unknown): value is number {
  return isFiniteNumber(value) && value >= 0 && value <= 1;
}

function readNumberMap(value: unknown): Record<string, number> | null {
  if (!isRecord(value)) return null;
  const entries = Object.entries(value);
  if (entries.length === 0 || entries.some(([, item]) => !isProbability(item))) {
    return null;
  }
  return Object.fromEntries(entries) as Record<string, number>;
}

function readStringMap(value: unknown): Record<string, string> | null {
  if (!isRecord(value)) return null;
  const entries = Object.entries(value);
  if (entries.length === 0 || entries.some(([, item]) => typeof item !== "string")) {
    return null;
  }
  return Object.fromEntries(entries) as Record<string, string>;
}

function parseAnswer(
  value: unknown,
  expectedType: Primitive,
): DecisionAnswer | null {
  if (!isRecord(value) || value.type !== expectedType) return null;

  if (expectedType === "noul") {
    if (value.noul === 0 || value.noul === 0.5 || value.noul === 1) {
      return { type: "noul", noul: value.noul };
    }
    return null;
  }

  if (expectedType === "choice") {
    const probabilities = readNumberMap(value.probabilities);
    if (
      typeof value.choice !== "string" ||
      value.choice.trim().length === 0 ||
      !probabilities ||
      !isProbability(value.confidence)
    ) {
      return null;
    }
    return {
      type: "choice",
      choice: value.choice,
      probabilities,
      confidence: value.confidence,
    };
  }

  if (expectedType === "score") {
    const legend = readStringMap(value.legend);
    const probabilities = readNumberMap(value.probabilities);
    if (
      !isFiniteNumber(value.score) ||
      !legend ||
      !probabilities ||
      !isProbability(value.confidence)
    ) {
      return null;
    }
    return {
      type: "score",
      score: value.score,
      legend,
      probabilities,
      confidence: value.confidence,
    };
  }

  if (!isRecord(value.box) || !isProbability(value.confidence)) return null;
  const { top, bottom, left, right } = value.box;
  if (
    !isProbability(top) ||
    !isProbability(bottom) ||
    !isProbability(left) ||
    !isProbability(right) ||
    top >= bottom ||
    left >= right
  ) {
    return null;
  }
  return {
    type: "box",
    box: { top, bottom, left, right },
    confidence: value.confidence,
  };
}

function parseDecision(
  value: unknown,
  preset: Primitive,
): ParsedDecision | null {
  if (
    !isRecord(value) ||
    typeof value.model !== "string" ||
    value.model.trim().length === 0 ||
    !isRecord(value.answers)
  ) {
    return null;
  }
  const answer = parseAnswer(value.answers[questionIds[preset]], preset);
  if (!answer) return null;

  if (!isRecord(value.usage)) return null;
  if (
    !isFiniteNumber(value.usage.input_tokens) ||
    !isFiniteNumber(value.usage.output_tokens) ||
    !isFiniteNumber(value.usage.cost)
  ) {
    return null;
  }
  return { model: value.model, answer };
}

function formatProbability(value: number): string {
  return `${(value * 100).toFixed(1)}%`;
}

function ProbabilityList({ values }: { values: Record<string, number> }) {
  const sorted = Object.entries(values).sort((left, right) => right[1] - left[1]);
  return (
    <ul className="probability-list">
      {sorted.map(([label, probability]) => (
        <li key={label}>
          <div className="probability-row">
            <span>{label}</span>
            <span>{formatProbability(probability)}</span>
          </div>
          <div className="probability-track" aria-hidden="true"><span style={{ width: `${probability * 100}%` }} /></div>
        </li>
      ))}
    </ul>
  );
}

function DecisionResult({ answer }: { answer: DecisionAnswer }) {
  if (answer.type === "noul") {
    const label = answer.noul === 1 ? "YES" : answer.noul === 0 ? "NO" : "UNDECIDED";
    return (
      <div className="decision-result">
        <span className="decision-value">{label} {answer.noul.toFixed(1)}</span>
      </div>
    );
  }

  if (answer.type === "choice") {
    return (
      <div className="decision-result">
        <span className="decision-value">{answer.confidence === 0 ? "No preference" : answer.choice}</span>
        {answer.confidence === 0 ? <p className="muted">All options are equally likely. The API returns {answer.choice} as its tie-break choice.</p> : null}
        <Badge tone={answer.confidence === 0 ? "warning" : "success"}>Confidence {formatProbability(answer.confidence)}</Badge>
        <ProbabilityList values={answer.probabilities} />
      </div>
    );
  }

  if (answer.type === "score") {
    const levelLabel = answer.legend[String(answer.score)];
    return (
      <div className="decision-result">
        <span className="decision-value">Level {answer.score}</span>
        <p className="muted">
          {answer.confidence === 0 ? "The midpoint of equally likely levels, not a selected winner." : levelLabel ?? "Between the listed score levels"}
        </p>
        <Badge tone={answer.confidence === 0 ? "warning" : "success"}>Confidence {formatProbability(answer.confidence)}</Badge>
        <ul className="score-legend">
          {Object.entries(answer.legend).map(([level, label]) => (
            <li key={level}>
              <span>Level {level}: {label}</span>
              {answer.confidence > 0 && Number(level) === answer.score ? <Badge tone="info">Selected</Badge> : null}
            </li>
          ))}
        </ul>
        <ProbabilityList values={answer.probabilities} />
      </div>
    );
  }

  const { top, bottom, left, right } = answer.box;
  return (
    <div className="decision-result">
      <Badge tone={answer.confidence === 0 ? "warning" : "success"}>Confidence {formatProbability(answer.confidence)}</Badge>
      <div
        className="box-preview"
        role="img"
        aria-label="Two to one normalized coordinate space with the returned box outlined. No source image is included."
      >
        <span className="box-preview-label">2:1 normalized coordinate space</span>
        <span
          className="box-overlay"
          style={{
            top: `${top * 100}%`,
            left: `${left * 100}%`,
            width: `${(right - left) * 100}%`,
            height: `${(bottom - top) * 100}%`,
          }}
          aria-hidden="true"
        />
      </div>
      <p className="muted">
        Normalized box: top {top.toFixed(3)}, bottom {bottom.toFixed(3)}, left {left.toFixed(3)}, right {right.toFixed(3)}.
      </p>
    </div>
  );
}

function animateCoin(element: HTMLElement | null, frames: Keyframe[], options: KeyframeAnimationOptions): Animation | null {
  try {
    return element?.animate?.(frames, options) ?? null;
  } catch {
    // A visual enhancement must never prevent the API request or its result.
    return null;
  }
}

export default function CoinPlayground() {
  const [preset, setPreset] = useState<Primitive>("noul");
  const [model, setModel] = useState<DecisionModel>("flip-latest");
  const [state, setState] = useState(defaultState);
  const [instructions, setInstructions] = useState(() => createRequest("noul").questions[questionIds.noul].instructions);
  const [phase, setPhase] = useState<"idle" | "requesting" | "landing">("idle");
  const loading = phase !== "idle";
  const [coinFace, setCoinFace] = useState<CoinFace>("obverse");
  const [coinRotation, setCoinRotation] = useState(0);
  const [completed, setCompleted] = useState<CompletedRun | null>(null);
  const [error, setError] = useState("");
  const pending = useRef(false);
  const coinElement = useRef<HTMLDivElement>(null);
  const motion = useRef<Animation | null>(null);
  const requestController = useRef<AbortController | null>(null);
  const mounted = useRef(true);

  useEffect(() => {
    mounted.current = true;
    const preference = window.matchMedia("(prefers-reduced-motion: reduce)");
    const stopMotion = () => { if (preference.matches) motion.current?.cancel(); };
    preference.addEventListener("change", stopMotion);
    return () => {
      mounted.current = false;
      requestController.current?.abort();
      motion.current?.cancel();
      preference.removeEventListener("change", stopMotion);
    };
  }, []);

  const flip = async () => {
    if (pending.current) return;
    pending.current = true;
    setPhase("requesting");
    setCompleted(null);
    setError("");
    const snapshot: RunSnapshot = { preset, model, state, instructions };
    const controller = new AbortController();
    requestController.current = controller;
    const coin = coinElement.current;
    const reduceMotion = () => window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    motion.current?.cancel();
    const spin = !reduceMotion() ? animateCoin(coin,
      [{ transform: `rotateY(${coinRotation}deg)` }, { transform: `rotateY(${coinRotation + 360}deg)` }],
      { duration: COIN_SPIN_MS, iterations: Infinity, easing: "linear" },
    ) : null;
    motion.current = spin;

    try {
      const request = createRequest(snapshot.preset, snapshot.model, snapshot.state);
      request.questions[questionIds[snapshot.preset]].instructions = snapshot.instructions;
      if (snapshot.preset === "noul") {
        request.questions[questionIds.noul].criteria = { true: "Yes", false: "No" };
      }
      const result = await requestJson({
        endpoint: "/v1/systemone",
        method: "POST",
        token: "banana",
        body: JSON.stringify(request),
        signal: controller.signal,
      });
      if (!mounted.current) return;
      // Publish the response immediately; only the physical landing is animated.
      setCompleted({ snapshot, result });
      const parsed = result.ok && result.hasJson ? parseDecision(result.data, snapshot.preset) : null;
      if (!parsed) {
        setError(!result.ok
          ? `Flip returned HTTP ${result.status}${result.statusText ? ` ${result.statusText}` : ""}. Review the response and try again.`
          : !result.hasJson
            ? result.parseError ?? "Flip returned a non-JSON response. Check the server and retry."
            : "Flip returned an unexpected response shape. Check the API response and retry.");
        return;
      }

      const answer = parsed.answer;
      const edge = answer.type === "noul" ? answer.noul === 0.5 : answer.confidence === 0;
      const nextFace: CoinFace = edge ? "edge" : answer.type === "noul" && answer.noul === 0 ? "reverse" : "obverse";
      const faceAngle = nextFace === "edge" ? 78 : nextFace === "reverse" ? 180 : 0;
      const elapsed = typeof spin?.currentTime === "number" ? spin.currentTime : 0;
      const currentAngle = coinRotation + elapsed / COIN_SPIN_MS * 360;
      const landingAngle = Math.ceil(currentAngle / 360) * 360 + 360 + faceAngle;
      spin?.cancel();
      setCoinFace(nextFace);
      setCoinRotation(landingAngle);
      setPhase("landing");
      if (coin && !reduceMotion()) {
        const landing = animateCoin(coin,
          [{ transform: `rotateY(${currentAngle}deg)` }, { transform: `rotateY(${landingAngle}deg)` }],
          { duration: COIN_LANDING_MS, easing: "cubic-bezier(.16, 1, .3, 1)", fill: "forwards" },
        );
        motion.current = landing;
        await landing?.finished.catch(() => undefined);
      }
    } catch (requestError) {
      if (!mounted.current) return;
      setError(requestError instanceof Error
        ? requestError.message
        : "Could not complete the request. Check the Flip server and retry.");
    } finally {
      motion.current?.cancel();
      motion.current = null;
      requestController.current = null;
      pending.current = false;
      if (mounted.current) setPhase("idle");
    }
  };

  const decision =
    completed?.result.ok && completed.result.hasJson
      ? parseDecision(completed.result.data, completed.snapshot.preset)
      : null;

  return (
    <SectionPanel
      id="coin-playground"
      tabIndex={-1}
      className="page-card coin-playground"
      title="Coin playground"
      description="Choose a decision type and model, then ask Flip for a live result."
    >
      <div className="playground-grid">
        <fieldset className="playground-controls panel-stack" disabled={loading} aria-label="Decision request" data-ui>
          <div className="control-group">
            <span className="control-label">Decision type</span>
            <SegmentedControl
              options={primitiveOptions}
              value={preset}
              onValueChange={(value) => {
                const next = value as Primitive;
                setPreset(next);
                setInstructions(createRequest(next).questions[questionIds[next]].instructions);
              }}
              label="Decision type"
            />
          </div>
          <div className="control-group">
            <span className="control-label">Coin</span>
            <SegmentedControl
              options={modelOptions}
              value={model}
              onValueChange={(value) => setModel(value as DecisionModel)}
              label="Coin model"
            />
          </div>
          <Input
            id="playground-question"
            label="Your question"
            value={instructions}
            onChange={(event) => setInstructions(event.currentTarget.value)}
            autoComplete="off"
          />
          <Input
            id="playground-state"
            label="Context"
            value={state}
            onChange={(event) => setState(event.currentTarget.value)}
            autoComplete="off"
          />
          <p className="muted playground-hint">{preset === "noul" ? "Heads means yes. Tails means no. The API decides the landing." : preset === "choice" ? "Options: billing, support, sales. Customize the options in the API tester." : preset === "score" ? "Levels: 0 Calm, 1 Frustrated, 2 Very frustrated." : "A random rectangle in normalized coordinates. No image is sent."}</p>
          <div className="action-row">
            <Button type="button" onClick={flip} loading={loading}>
              Flip it
            </Button>
            <span className="muted" role="status" aria-live="polite">
              {phase === "requesting" ? "Request sent. Waiting for the API…" : phase === "landing" ? "Response received. Landing the coin…" : error ? "Decision failed. Review the error below." : decision ? `Decision ready: ${decision.answer.type === "noul" ? decision.answer.noul === 1 ? "Yes" : decision.answer.noul === 0 ? "No" : "Undecided" : decision.answer.type === "choice" ? decision.answer.confidence === 0 ? "No preference" : decision.answer.choice : decision.answer.type === "score" ? `Level ${decision.answer.score}` : "Box returned"}.` : ""}
            </span>
          </div>
        </fieldset>

        <div className="coin-stage">
          <div
            ref={coinElement}
            className={`coin${loading ? " is-flipping" : ""}`}
            data-phase={phase}
            aria-hidden="true"
            data-face={coinFace}
            style={{ transform: `rotateY(${coinRotation}deg)` }}
          >
            <div className="coin-photo hadrian-photo coin-face coin-face--obverse">
              <img src={hadrian} width="1600" height="889" alt="" loading="lazy" />
            </div>
            <div className="coin-photo hadrian-photo coin-face coin-face--reverse">
              <img src={hadrian} width="1600" height="889" alt="" loading="lazy" />
            </div>
          </div>
          <p className="coin-stage-caption">{phase === "requesting" ? "Waiting for the live API response." : phase === "landing" ? "Landing on the returned answer." : decision ? coinFace === "edge" ? "On edge. Certainty takes the day off." : decision.answer.type === "noul" ? coinFace === "obverse" ? "Heads. Yes." : "Tails. No." : "A live API result. Coin face is illustrative." : "Your question. Its complete lack of judgment."}</p>
        </div>
      </div>

      {error ? (
        <Alert data-ui tone="danger" title="Decision request needs attention">
          {error}
        </Alert>
      ) : null}

      {completed ? (
        <section className="panel-stack" aria-label="Decision result">
          <div className="response-meta" data-ui>
            <Badge tone={completed.result.ok ? "success" : "danger"}>
              {completed.result.status} {completed.result.statusText || "HTTP"}
            </Badge>
            <span className="muted">{completed.result.elapsedMs} ms</span>
            <span className="muted">
              Result for {completed.snapshot.model} / {completed.snapshot.preset}, using the captured request.
            </span>
            {decision ? <Badge tone="info">Server model {decision.model}</Badge> : null}
          </div>

          <p className="captured-question">{completed.snapshot.instructions}</p>
          {decision ? <div data-ui><DecisionResult answer={decision.answer} /></div> : null}

          {completed.result.hasJson ? (
            <details className="response-disclosure"><summary>Inspect API response</summary><div data-ui><CodeBlock
              code={JSON.stringify(completed.result.data, null, 2) ?? "null"}
              language="json"
              filename="response.json"
            /></div></details>
          ) : null}
        </section>
      ) : phase === "requesting" ? (
        <div className="empty-result pending-result" aria-busy="true">
          <p>Request in flight.</p>
          <span className="muted">The coin is spinning while the API decides. The returned answer determines its landing.</span>
        </div>
      ) : !error ? (
        <div className="empty-result">
          <p>Your next decision starts here.</p>
          <span className="muted">Ask a question, then flip the coin for a live answer.</span>
        </div>
      ) : null}
    </SectionPanel>
  );
}
