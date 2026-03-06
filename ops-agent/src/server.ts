import { createServer, type IncomingMessage, type ServerResponse } from "node:http";
import { resolve } from "node:path";
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

async function main(): Promise<void> {
  const port = Number(process.env.OPS_AGENT_PORT ?? 8787);
  const baseCwd = process.env.OPS_AGENT_CWD ? resolve(process.env.OPS_AGENT_CWD) : process.cwd();

  const metrics = createMetrics();
  const store = new PlanStore(resolve(baseCwd, "ops-agent/state/store.json"));
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

      if (req.method === "GET" && req.url === "/healthz") {
        sendJson(res, 200, { status: "ok" });
        return;
      }

      if (req.method === "GET" && req.url === "/metrics") {
        const body = await metrics.registry.metrics();
        sendText(res, 200, body);
        return;
      }

      if (req.method === "POST" && req.url === "/v1/plan") {
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

      if (req.method === "GET" && req.url.startsWith("/v1/plans/")) {
        const match = req.url.match(/^\/v1\/plans\/([^/]+)$/);
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

      if (req.method === "POST" && req.url === "/v1/approve") {
        const parsed = validateBody<ApproveRequest>(ApproveRequestSchema, await readJsonBody(req));
        const approval = orchestrator.approve(parsed.planId, parsed.commandHashes, parsed.ttlSeconds ?? 900);
        sendJson(res, 200, {
          approvalToken: approval.token,
          expiresAt: approval.expiresAt,
        });
        return;
      }

      if (req.method === "POST" && req.url === "/v1/execute") {
        const parsed = validateBody<ExecuteRequest>(ExecuteRequestSchema, await readJsonBody(req));
        const execution = await orchestrator.execute(parsed.planId, {
          approvalToken: parsed.approvalToken,
          readOnly: parsed.readOnly,
        });
        sendJson(res, 202, execution);
        return;
      }

      if (req.method === "GET" && req.url.startsWith("/v1/runs/")) {
        const match = req.url.match(/^\/v1\/runs\/([^/]+)(\/events)?$/);
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

  server.listen(port, "0.0.0.0", () => {
    // eslint-disable-next-line no-console
    console.log(`ops-agent listening on :${port}`);
  });
}

void main();
