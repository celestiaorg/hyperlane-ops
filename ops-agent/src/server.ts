import { createServer, type IncomingMessage, type ServerResponse } from "node:http";
import { existsSync, readFileSync } from "node:fs";
import { homedir } from "node:os";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";
import {
  ApproveRequestSchema,
  type ApproveRequest,
  ExecuteRequestSchema,
  type ExecuteRequest,
  PlanRequestSchema,
  type PlanRequest,
} from "./api-schema.js";
import { PiDecisionEngine } from "./decision-engine.js";
import { createMetrics } from "./metrics.js";
import { OpsOrchestrator } from "./orchestrator.js";
import { PiRpcClient } from "./pi-rpc-client.js";
import { PlanStore } from "./plan-store.js";
import { validateBody } from "./validation.js";

function readJsonBody(req: IncomingMessage): Promise<unknown> {
  return new Promise((resolveBody, reject) => {
    let raw = "";

    req.on("data", (chunk) => {
      raw += chunk.toString();
      if (raw.length > 1_000_000) {
        reject(new Error("Request body too large"));
      }
    });

    req.on("end", () => {
      if (!raw.trim()) {
        resolveBody({});
        return;
      }

      try {
        resolveBody(JSON.parse(raw));
      } catch {
        reject(new Error("Invalid JSON body"));
      }
    });

    req.on("error", (error) => reject(error));
  });
}

function sendJson(res: ServerResponse, statusCode: number, body: unknown): void {
  res.writeHead(statusCode, {
    "Content-Type": "application/json",
  });
  res.end(`${JSON.stringify(body)}\n`);
}

function sendText(res: ServerResponse, statusCode: number, body: string): void {
  res.writeHead(statusCode, {
    "Content-Type": "text/plain; charset=utf-8",
  });
  res.end(body);
}

function hasEnv(name: string): boolean {
  return Boolean(process.env[name] && process.env[name]?.trim().length);
}

function authFilePath(): string {
  if (process.env.PI_AUTH_FILE && process.env.PI_AUTH_FILE.trim().length > 0) {
    return process.env.PI_AUTH_FILE;
  }
  return resolve(homedir(), ".pi/agent/auth.json");
}

function resolveStateStorePath(baseCwd: string): string {
  if (process.env.OPS_AGENT_STATE_FILE && process.env.OPS_AGENT_STATE_FILE.trim().length > 0) {
    return resolve(baseCwd, process.env.OPS_AGENT_STATE_FILE.trim());
  }

  const monorepoOpsAgentDir = resolve(baseCwd, "ops-agent");
  if (existsSync(resolve(monorepoOpsAgentDir, "package.json"))) {
    return resolve(monorepoOpsAgentDir, "state/store.json");
  }

  return resolve(baseCwd, "state/store.json");
}

function authFileInfo(filePath: string): {
  present: boolean;
  hasOpenAiApiKey: boolean;
  hasOpenAiCodexOAuth: boolean;
  parseError?: string;
} {
  if (!existsSync(filePath)) {
    return {
      present: false,
      hasOpenAiApiKey: false,
      hasOpenAiCodexOAuth: false,
    };
  }

  try {
    const parsed = JSON.parse(readFileSync(filePath, "utf8")) as Record<string, unknown>;
    const openai = parsed.openai;
    const openaiCodex = parsed["openai-codex"];

    const hasOpenAiApiKey =
      !!openai && typeof openai === "object" && (openai as { type?: string }).type === "api_key";
    const hasOpenAiCodexOAuth =
      !!openaiCodex && typeof openaiCodex === "object" && (openaiCodex as { type?: string }).type === "oauth";

    return {
      present: true,
      hasOpenAiApiKey,
      hasOpenAiCodexOAuth,
    };
  } catch (error) {
    return {
      present: true,
      hasOpenAiApiKey: false,
      hasOpenAiCodexOAuth: false,
      parseError: error instanceof Error ? error.message : String(error),
    };
  }
}

async function verifyPi(baseCwd: string): Promise<{ ok: boolean; provider: string; model: string; error?: string }> {
  const provider = process.env.PI_DEFAULT_PROVIDER ?? "";
  const model = process.env.PI_DEFAULT_MODEL ?? "";
  const client = new PiRpcClient({
    cwd: baseCwd,
    provider: provider || undefined,
    model: model || undefined,
    sessionDir: process.env.PI_SESSION_DIR,
    timeoutMs: 30_000,
  });

  try {
    await client.start();
    await client.promptAndWait(
      "Reply with a JSON object only: {\"status\":\"ok\"}",
      30_000,
    );
    const text = await client.getLastAssistantText();
    if (!text || !text.trim().length) {
      throw new Error("Pi returned empty response");
    }
    return { ok: true, provider, model };
  } catch (error) {
    return {
      ok: false,
      provider,
      model,
      error: error instanceof Error ? error.message : String(error),
    };
  } finally {
    await client.stop();
  }
}

export async function startServer(): Promise<void> {
  const port = Number(process.env.OPS_AGENT_PORT ?? 8787);
  const baseCwd = process.env.OPS_AGENT_CWD ? resolve(process.env.OPS_AGENT_CWD) : process.cwd();

  const metrics = createMetrics();
  const store = new PlanStore(resolveStateStorePath(baseCwd));
  const orchestrator = new OpsOrchestrator({
    baseCwd,
    decisionEngine: new PiDecisionEngine(),
    store,
    metrics,
  });

  const server = createServer(async (req, res) => {
    try {
      if (!req.url || !req.method) {
        sendJson(res, 400, { error: "Invalid request" });
        return;
      }

      const parsedUrl = new URL(req.url, "http://localhost");
      const pathname = parsedUrl.pathname;

      if (req.method === "GET" && pathname === "/healthz") {
        sendJson(res, 200, { status: "ok" });
        return;
      }

      if (req.method === "GET" && pathname === "/metrics") {
        const body = await metrics.registry.metrics();
        sendText(res, 200, body);
        return;
      }

      if (req.method === "GET" && pathname === "/v1/info") {
        const verify = parsedUrl.searchParams.get("verify") === "1" || parsedUrl.searchParams.get("verify") === "true";
        const authPath = authFilePath();
        const authInfo = authFileInfo(authPath);

        const payload: {
          model: {
            provider: string;
            model: string;
          };
          credentials: {
            openaiApiKeyPresent: boolean;
            hypKeyPresent: boolean;
            hypKeyCosmosnativePresent: boolean;
            authFilePath: string;
            authFilePresent: boolean;
            authFileHasOpenAiApiKey: boolean;
            authFileHasOpenAiCodexOAuth: boolean;
            authFileParseError?: string;
          };
          runtime: {
            cwd: string;
            sessionDir: string;
          };
          verification?: {
            requested: true;
            ok: boolean;
            provider: string;
            model: string;
            error?: string;
          };
        } = {
          model: {
            provider: process.env.PI_DEFAULT_PROVIDER ?? "",
            model: process.env.PI_DEFAULT_MODEL ?? "",
          },
          credentials: {
            openaiApiKeyPresent: hasEnv("OPENAI_API_KEY"),
            hypKeyPresent: hasEnv("HYP_KEY"),
            hypKeyCosmosnativePresent: hasEnv("HYP_KEY_COSMOSNATIVE"),
            authFilePath: authPath,
            authFilePresent: authInfo.present,
            authFileHasOpenAiApiKey: authInfo.hasOpenAiApiKey,
            authFileHasOpenAiCodexOAuth: authInfo.hasOpenAiCodexOAuth,
            authFileParseError: authInfo.parseError,
          },
          runtime: {
            cwd: baseCwd,
            sessionDir: process.env.PI_SESSION_DIR ?? "",
          },
        };

        if (verify) {
          payload.verification = {
            requested: true,
            ...(await verifyPi(baseCwd)),
          };
        }

        sendJson(res, 200, payload);
        return;
      }

      if (req.method === "POST" && pathname === "/v1/plan") {
        const parsed = validateBody<PlanRequest>(PlanRequestSchema, await readJsonBody(req));
        const result = await orchestrator.createPlan(parsed.goal, parsed.context, parsed.cwd);
        sendJson(res, 200, {
          planId: result.plan.id,
          summary: result.plan.summary,
          commands: result.plan.commands,
          rawResponse: result.rawResponse,
        });
        return;
      }

      if (req.method === "GET" && pathname.startsWith("/v1/plans/")) {
        const match = pathname.match(/^\/v1\/plans\/([^/]+)$/);
        if (!match) {
          sendJson(res, 404, { error: "Not found" });
          return;
        }

        const plan = orchestrator.getPlan(match[1]);
        if (!plan) {
          sendJson(res, 404, { error: "Plan not found" });
          return;
        }

        sendJson(res, 200, plan);
        return;
      }

      if (req.method === "POST" && pathname === "/v1/approve") {
        const parsed = validateBody<ApproveRequest>(ApproveRequestSchema, await readJsonBody(req));
        const approval = orchestrator.approve(parsed.planId, parsed.commandHashes, parsed.ttlSeconds ?? 900);
        sendJson(res, 200, {
          approvalToken: approval.token,
          expiresAt: approval.expiresAt,
        });
        return;
      }

      if (req.method === "POST" && pathname === "/v1/execute") {
        const parsed = validateBody<ExecuteRequest>(ExecuteRequestSchema, await readJsonBody(req));
        const execution = await orchestrator.execute(parsed.planId, {
          approvalToken: parsed.approvalToken,
          readOnly: parsed.readOnly,
        });
        sendJson(res, 202, execution);
        return;
      }

      if (req.method === "GET" && pathname.startsWith("/v1/runs/")) {
        const match = pathname.match(/^\/v1\/runs\/([^/]+)(\/events)?$/);
        if (!match) {
          sendJson(res, 404, { error: "Not found" });
          return;
        }

        const runId = match[1];
        const run = orchestrator.getRun(runId);
        if (!run) {
          sendJson(res, 404, { error: "Run not found" });
          return;
        }

        if (match[2]) {
          const ndjson = `${run.transcript.map((event) => JSON.stringify(event)).join("\n")}\n`;
          sendText(res, 200, ndjson);
          return;
        }

        sendJson(res, 200, run);
        return;
      }

      sendJson(res, 404, { error: "Not found" });
    } catch (error) {
      sendJson(res, 400, {
        error: error instanceof Error ? error.message : String(error),
      });
    }
  });

  await new Promise<void>((resolveListen, rejectListen) => {
    const onError = (error: Error) => {
      server.off("error", onError);
      rejectListen(error);
    };

    server.once("error", onError);
    server.listen(port, "0.0.0.0", () => {
      server.off("error", onError);
      // eslint-disable-next-line no-console
      console.log(`ops-agent listening on :${port}`);
      resolveListen();
    });
  });
}

async function main(): Promise<void> {
  await startServer();
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  void main();
}
