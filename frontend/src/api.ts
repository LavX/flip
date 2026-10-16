export type RequestMethod = "GET" | "POST";

export interface JsonRequestOptions {
  endpoint: string;
  method: RequestMethod;
  token: string;
  body?: string;
  signal?: AbortSignal;
}

export interface JsonRequestResult {
  status: number;
  statusText: string;
  ok: boolean;
  elapsedMs: number;
  hasJson: boolean;
  data: unknown;
  parseError?: string;
}

export class RequestFailure extends Error {
  readonly kind: "network" | "timeout";

  constructor(kind: "network" | "timeout", message: string) {
    super(message);
    this.name = "RequestFailure";
    this.kind = kind;
  }
}

const REQUEST_TIMEOUT_MS = 20_000;

export async function requestJson({
  endpoint,
  method,
  token,
  body,
  signal,
}: JsonRequestOptions): Promise<JsonRequestResult> {
  const controller = new AbortController();
  const abort = () => controller.abort();
  signal?.addEventListener("abort", abort, { once: true });
  if (signal?.aborted) controller.abort();
  const startedAt = performance.now();
  let timedOut = false;
  const timeoutId = setTimeout(() => {
    timedOut = true;
    controller.abort();
  }, REQUEST_TIMEOUT_MS);

  try {
    const headers = new Headers({
      Accept: "application/json",
      Authorization: `Bearer ${token}`,
    });
    if (method === "POST") headers.set("Content-Type", "application/json");

    const response = await fetch(endpoint, {
      method,
      headers,
      ...(method === "POST" ? { body: body ?? "" } : {}),
      signal: controller.signal,
    });
    const responseText = await response.text();
    const elapsedMs = Math.round(performance.now() - startedAt);

    try {
      return {
        status: response.status,
        statusText: response.statusText,
        ok: response.ok,
        elapsedMs,
        hasJson: true,
        data: JSON.parse(responseText) as unknown,
      };
    } catch {
      return {
        status: response.status,
        statusText: response.statusText,
        ok: response.ok,
        elapsedMs,
        hasJson: false,
        data: undefined,
        parseError:
          "Flip returned a non-JSON response. Check that the selected endpoint is served by Flip, then retry.",
      };
    }
  } catch (error) {
    if (timedOut) {
      throw new RequestFailure(
        "timeout",
        `The request to ${endpoint} took longer than 20 seconds. Check the Flip server and retry.`,
      );
    }

    const detail = error instanceof Error ? ` ${error.message}` : "";
    throw new RequestFailure(
      "network",
      `Could not reach ${endpoint}.${detail} Check that Flip is running and retry.`,
    );
  } finally {
    clearTimeout(timeoutId);
    signal?.removeEventListener("abort", abort);
  }
}

export interface CurlCommandOptions extends JsonRequestOptions {
  origin: string;
}

function shellQuote(value: string): string {
  return `'${value.replace(/'/g, "'\\''")}'`;
}

export function buildCurlCommand({
  origin,
  endpoint,
  method,
  token,
  body,
}: CurlCommandOptions): string {
  const url = `${origin.replace(/\/+$/, "")}${endpoint}`;
  const parts = [
    "curl",
    "--request",
    shellQuote(method),
    "--header",
    shellQuote("Accept: application/json"),
    "--header",
    shellQuote(`Authorization: Bearer ${token}`),
  ];

  if (method === "POST") {
    parts.push(
      "--header",
      shellQuote("Content-Type: application/json"),
      "--data-raw",
      shellQuote(body ?? ""),
    );
  }

  parts.push(shellQuote(url));
  return parts.join(" ");
}
