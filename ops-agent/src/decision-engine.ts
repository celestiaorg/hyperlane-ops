import { existsSync } from "node:fs";
import { resolve } from "node:path";
import type { DecisionEngine, DecisionPlan } from "./types.js";
import { PiRpcClient } from "./pi-rpc-client.js";

interface RawPlan {
  summary: string;
  commands: string[];
}

function extractJsonPayload(text: string): RawPlan {
  const trimmed = text.trim();
  const fenced = trimmed.match(/```json\s*([\s\S]*?)\s*```/i);
  const payload = fenced ? fenced[1] : trimmed;

  const parsed = JSON.parse(payload) as RawPlan;

  if (!parsed.summary || !Array.isArray(parsed.commands)) {
    throw new Error("Plan output missing summary or commands");
  }

  if (!parsed.commands.every((item) => typeof item === "string")) {
    throw new Error("Plan commands must be strings");
  }

  return parsed;
}

function resolvePiRuntimeCwd(commandCwd: string): string {
  if (process.env.PI_PROJECT_CWD && process.env.PI_PROJECT_CWD.trim().length > 0) {
    return resolve(process.env.PI_PROJECT_CWD.trim());
  }

  const candidates = [commandCwd, resolve(commandCwd, ".."), resolve(commandCwd, "ops-agent")];
  const visited = new Set<string>();
  for (const candidate of candidates) {
    const normalized = resolve(candidate);
    if (visited.has(normalized)) {
      continue;
    }
    visited.add(normalized);
    if (existsSync(resolve(normalized, ".pi/settings.json"))) {
      return normalized;
    }
  }

  return commandCwd;
}

function makePlannerPrompt(goal: string, commandCwd: string, context?: string): string {
  const contextText = context?.trim().length ? `Context:\n${context}\n` : "";

  return [
    "You are an ops command planner for a Hyperlane operations repository.",
    "Create a concise execution plan as strict JSON with this shape:",
    '{"summary":"...","commands":["..."]}',
    "Rules:",
    "- Commands must be shell commands only.",
    "- Prefer read-only checks first, then mutating commands if needed.",
    "- Use --registry . for hyperlane core/warp commands.",
    "- Use only these workflow families: hyperlane core, hyperlane warp, docker compose relayer, cast call/send, celestia-appd query/tx.",
    `- Commands are executed from this working directory: ${commandCwd}`,
    "- No prose outside JSON.",
    contextText,
    `Goal: ${goal}`,
  ].join("\n");
}

export class PiDecisionEngine implements DecisionEngine {
  async createPlan(goal: string, context: string | undefined, cwd: string): Promise<DecisionPlan> {
    const piRuntimeCwd = resolvePiRuntimeCwd(cwd);
    const client = new PiRpcClient({
      cwd: piRuntimeCwd,
      provider: process.env.PI_DEFAULT_PROVIDER,
      model: process.env.PI_DEFAULT_MODEL,
      sessionDir: process.env.PI_SESSION_DIR,
      timeoutMs: Number(process.env.PI_RPC_TIMEOUT_MS ?? 120_000),
    });

    try {
      await client.start();
      const prompt = makePlannerPrompt(goal, cwd, context);
      let waitError: Error | undefined;
      try {
        await client.promptAndWait(prompt);
      } catch (error) {
        waitError = error instanceof Error ? error : new Error(String(error));
      }
      const assistantText = await client.getLastAssistantText();
      if (!assistantText) {
        if (waitError) {
          throw waitError;
        }
        throw new Error("Pi returned no assistant message");
      }
      const parsed = extractJsonPayload(assistantText);

      return {
        summary: parsed.summary,
        commands: parsed.commands,
        rawResponse: assistantText,
      };
    } finally {
      await client.stop();
    }
  }
}

export class FallbackDecisionEngine implements DecisionEngine {
  async createPlan(goal: string, _context: string | undefined, _cwd: string): Promise<DecisionPlan> {
    const lower = goal.toLowerCase();
    const commands: string[] = [];

    if (lower.includes("relayer") || lower.includes("health")) {
      commands.push("docker compose ps relayer");
      commands.push("docker logs hyperlane-relayer --tail=200");
    }

    if (lower.includes("warp")) {
      commands.push("hyperlane warp read --symbol USDC --registry .");
    }

    if (lower.includes("core")) {
      commands.push("hyperlane core read --chain sepolia --config configs/sepolia-core.yaml --registry .");
    }

    if (!commands.length) {
      commands.push("docker compose ps relayer");
      commands.push("hyperlane registry list --registry .");
    }

    return {
      summary: "Fallback command plan based on repository runbooks",
      commands,
    };
  }
}
