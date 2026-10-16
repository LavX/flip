import { useRef, useState } from "react";
import { createRequest, pretty, type Preset } from "./data";
import { Alert } from "@/registry/components/alert/alert";
import { Badge } from "@/registry/components/badge/badge";
import { Button } from "@/registry/components/button/button";
import SectionPanel from "./SectionPanel";
import { CodeBlock } from "@/registry/components/code-block/code-block";
import { Input } from "@/registry/components/input/input";
import SegmentedControl, {
  type Segment,
} from "@/registry/components/segmented-control/segmented-control";
import { Textarea } from "@/registry/components/textarea/textarea";
import {
  buildCurlCommand,
  requestJson,
  RequestFailure,
  type JsonRequestResult,
  type RequestMethod,
} from "./api";

type Endpoint = "/v1/systemone" | "/api/alpha/decisions" | "/v1/models";
type PresetName = Preset;

interface RequestSnapshot {
  endpoint: Endpoint;
  method: RequestMethod;
  body?: string;
}

interface CompletedRequest {
  snapshot: RequestSnapshot;
  result: JsonRequestResult;
}

interface RequestAlert {
  title: string;
  message: string;
  snapshot?: RequestSnapshot;
}

const endpointOptions: Segment[] = [
  { value: "/v1/systemone", label: "POST /v1/systemone" },
  { value: "/api/alpha/decisions", label: "POST /api/alpha/decisions" },
  { value: "/v1/models", label: "GET /v1/models" },
];

const presets: PresetName[] = ["noul", "choice", "score", "box", "mixed"];

export default function ApiTester() {
  const [endpoint, setEndpoint] = useState<Endpoint>("/v1/systemone");
  const [token, setToken] = useState("banana");
  const [preset, setPreset] = useState<PresetName>("noul");
  const [body, setBody] = useState(() => pretty(createRequest("noul")));
  const [completed, setCompleted] = useState<CompletedRequest | null>(null);
  const [requestAlert, setRequestAlert] = useState<RequestAlert | null>(null);
  const [loading, setLoading] = useState(false);
  const [copying, setCopying] = useState(false);
  const [copyMessage, setCopyMessage] = useState("");
  const sendPending = useRef(false);
  const copyPending = useRef(false);
  const isGet = endpoint === "/v1/models";
  const method: RequestMethod = isGet ? "GET" : "POST";

  const loadPreset = (nextPreset: PresetName) => {
    setPreset(nextPreset);
    setBody(pretty(createRequest(nextPreset)));
  };

  const sendRequest = async () => {
    if (sendPending.current) return;
    if (!token.trim()) {
      setRequestAlert({
        title: "Bearer token required",
        message: "Enter any non-empty token. Flip accepts banana.",
      });
      return;
    }

    sendPending.current = true;
    setLoading(true);
    setCompleted(null);
    setRequestAlert(null);
    setCopyMessage("");
    const snapshot: RequestSnapshot = {
      endpoint,
      method,
      ...(isGet ? {} : { body }),
    };

    try {
      const result = await requestJson({
        endpoint: snapshot.endpoint,
        method: snapshot.method,
        token: token.trim(),
        ...(snapshot.body === undefined ? {} : { body: snapshot.body }),
      });
      setCompleted({ snapshot, result });
    } catch (error) {
      setRequestAlert({
        title:
          error instanceof RequestFailure
            ? error.kind === "timeout"
              ? "Request timed out"
              : "Could not reach Flip"
            : "Request failed",
        message:
          error instanceof Error
            ? error.message
            : "Could not complete the request. Check the Flip server and retry.",
        snapshot,
      });
    } finally {
      sendPending.current = false;
      setLoading(false);
    }
  };

  const copyCurl = async () => {
    if (copyPending.current) return;
    if (!token.trim()) {
      setCopyMessage("");
      setRequestAlert({
        title: "Bearer token required",
        message: "Enter any non-empty token before copying a runnable command.",
      });
      return;
    }

    copyPending.current = true;
    setCopying(true);
    setCopyMessage("");
    setRequestAlert(null);
    try {
      if (!navigator.clipboard?.writeText) {
        throw new Error("Clipboard access is unavailable in this browser.");
      }
      const command = buildCurlCommand({
        origin: window.location.origin,
        endpoint,
        method,
        token: token.trim(),
        ...(isGet ? {} : { body }),
      });
      await navigator.clipboard.writeText(command);
      setCopyMessage("Curl command copied.");
    } catch (error) {
      const detail = error instanceof Error ? error.message : "Clipboard access failed.";
      setRequestAlert({
        title: "Could not copy the curl command",
        message: `${detail} Check browser clipboard permissions, then try again.`,
      });
    } finally {
      copyPending.current = false;
      setCopying(false);
    }
  };

  return (
    <SectionPanel
      id="api-tester"
      tabIndex={-1}
      className="page-card api-tester"
      title="API tester"
      description="Send a request to the local Flip API and inspect its real response."
    >
      <p className="sr-only" role="status" aria-live="polite" aria-atomic="true">
        {loading ? "Request in progress." : completed?.result.ok
          ? `Request completed. HTTP ${completed.result.status}. ${completed.result.elapsedMs} milliseconds.`
          : ""}
      </p>
      <div className="panel-stack">
        <div className="field-row" data-ui>
          <div className="control-group">
            <span className="control-label">Endpoint</span>
            <SegmentedControl
              options={endpointOptions}
              value={endpoint}
              onValueChange={(value) => setEndpoint(value as Endpoint)}
              label="Endpoint"
            />
          </div>
          <div className="control-group">
            <Input
              id="api-bearer-token"
              label="Bearer token (any non-empty value; Flip accepts banana)"
              value={token}
              onChange={(event) => setToken(event.currentTarget.value)}
              autoComplete="off"
              spellCheck={false}
              required
            />
          </div>
        </div>

        <div className="tester-columns">
        <section data-ui className="tester-request" aria-label="Request">
          <div className="field-row">
            <div className="control-group">
              <span className="control-label">Quickstart presets</span>
              <div className="preset-row" role="group" aria-label="Load request preset">
                {presets.map((name) => (
                  <Button
                    key={name}
                    type="button"
                    variant={preset === name ? "secondary" : "ghost"}
                    size="sm"
                    aria-pressed={preset === name}
                    disabled={isGet}
                    onClick={() => loadPreset(name)}
                  >
                    {name}
                  </Button>
                ))}
              </div>
            </div>
          </div>

          <Textarea
            id="api-request-body"
            label="Request body"
            description={
              isGet
                ? "GET /v1/models sends no request body."
                : "JSON is sent as written so Flip can report malformed body errors."
            }
            className="request-editor"
            value={body}
            onChange={(event) => setBody(event.currentTarget.value)}
            disabled={isGet}
            spellCheck={false}
            autoCapitalize="off"
            autoCorrect="off"
          />

          <div className="action-row">
            <Button
              type="button"
              onClick={sendRequest}
              loading={loading}
              disabled={!token.trim()}
            >
              Send
            </Button>
            <Button
              type="button"
              variant="secondary"
              onClick={copyCurl}
              loading={copying}
              disabled={!token.trim()}
            >
              Copy as curl
            </Button>
            {copyMessage ? (
              <span className="muted" role="status">
                {copyMessage}
              </span>
            ) : null}
          </div>
        </section>

          <section data-ui className="tester-response" aria-label="API response">
            <h3>Response</h3>
            {requestAlert ? (
              <Alert tone="danger" title={requestAlert.title}>
                {requestAlert.message}
                {requestAlert.snapshot ? (
                  <span>
                    {" "}Request: {requestAlert.snapshot.method} {requestAlert.snapshot.endpoint}.
                  </span>
                ) : null}
              </Alert>
            ) : null}

            {completed ? (
              <>
                <div className="response-meta">
                  <Badge tone={completed.result.ok ? "success" : "danger"}>
                    {completed.result.status} {completed.result.statusText || "HTTP"}
                  </Badge>
                  <span className="muted">{completed.result.elapsedMs} ms</span>
                  <span className="muted">
                    {completed.snapshot.method} {completed.snapshot.endpoint}
                  </span>
                </div>
                {!completed.result.hasJson ? (
                  <Alert tone="danger" title="Response was not JSON">
                    {completed.result.parseError ?? "Check the endpoint and retry."}
                  </Alert>
                ) : !completed.result.ok ? (
                  <Alert tone="danger" title="Flip returned an HTTP error">
                    Review the status and JSON response below, correct the request, then retry.
                  </Alert>
                ) : null}
                {completed.result.hasJson ? (
                  <CodeBlock
                    code={pretty(completed.result.data)}
                    language="json"
                    filename="response.json"
                  />
                ) : null}
              </>
            ) : requestAlert ? null : (
              <div className="empty-result" role="status" aria-live="polite">
                <p>{loading ? "Waiting for Flip..." : "No response yet."}</p>
                <span className="muted">
                  {loading
                    ? "The response will appear here when the request finishes."
                    : "Send a request to see its HTTP status, latency, and JSON response here."}
                </span>
              </div>
            )}
          </section>
        </div>
      </div>
    </SectionPanel>
  );
}
