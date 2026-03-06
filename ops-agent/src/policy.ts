import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { looksLikeWriteToSensitiveFile, normalizeCommand, splitCommandSegments } from "./command-parser.js";
import type { PolicyEvaluation } from "./types.js";

const DESTRUCTIVE_PATTERNS = [/\bgit\s+reset\s+--hard\b/i, /\bgit\s+checkout\s+--\b/i, /\brm\s+-rf\b/i];

const READ_PATTERNS: RegExp[] = [
  /^hyperlane\s+core\s+read\b/i,
  /^hyperlane\s+warp\s+read\b/i,
  /^hyperlane\s+registry\s+list\b/i,
  /^cast\s+call\b/i,
  /^celestia-appd\s+query\b/i,
  /^docker\s+compose\s+ps\b/i,
  /^docker\s+logs\b/i,
];

const WRITE_PATTERNS: RegExp[] = [
  /^hyperlane\s+core\s+(deploy|apply)\b/i,
  /^hyperlane\s+warp\s+(deploy|apply)\b/i,
  /^cast\s+send\b/i,
  /^celestia-appd\s+tx\b/i,
  /^docker\s+compose\s+(restart|up|down)\b/i,
];

function getFlagValue(command: string, ...flags: string[]): string | undefined {
  const tokens = command.split(/\s+/);
  for (let i = 0; i < tokens.length; i += 1) {
    if (flags.includes(tokens[i]) && tokens[i + 1]) {
      return tokens[i + 1];
    }
  }
  return undefined;
}

function requiresRegistry(command: string): boolean {
  return /^hyperlane\s+(core|warp)\s+/i.test(command);
}

function hasRegistryFlag(command: string): boolean {
  return /--registry\s+\./.test(command);
}

function configImpliesCosmosNative(command: string, cwd: string): boolean {
  const cfgPath =
    getFlagValue(command, "--config") ?? getFlagValue(command, "--wd") ?? getFlagValue(command, "--wc");
  if (!cfgPath) {
    return false;
  }

  try {
    const resolved = resolve(cwd, cfgPath);
    const contents = readFileSync(resolved, "utf8").toLowerCase();
    return contents.includes("celestia") || contents.includes("cosmosnative");
  } catch {
    return false;
  }
}

function commandTargetsCosmosNative(command: string, cwd: string): boolean {
  const lower = command.toLowerCase();

  if (lower.includes("celestia") || lower.includes("mocha") || lower.includes("cosmosnative")) {
    return true;
  }

  return configImpliesCosmosNative(command, cwd);
}

function hasEnv(name: string): boolean {
  return Boolean(process.env[name] && process.env[name]?.trim().length);
}

export function evaluateCommand(command: string, cwd: string): PolicyEvaluation {
  const normalized = normalizeCommand(command);

  if (!normalized) {
    return {
      class: "blocked",
      reason: "Empty command",
      requiresApproval: false,
    };
  }

  if (DESTRUCTIVE_PATTERNS.some((pattern) => pattern.test(normalized))) {
    return {
      class: "blocked",
      reason: "Destructive command class is blocked",
      remediation: "Use targeted non-destructive commands instead",
      requiresApproval: false,
    };
  }

  if (looksLikeWriteToSensitiveFile(normalized)) {
    return {
      class: "blocked",
      reason: "Writes to sensitive files are blocked",
      remediation: "Store secrets in environment variables and never write .env via the agent",
      requiresApproval: false,
    };
  }

  if (requiresRegistry(normalized) && !hasRegistryFlag(normalized)) {
    return {
      class: "blocked",
      reason: "Hyperlane commands must include --registry .",
      remediation: `${normalized} --registry .`,
      requiresApproval: false,
    };
  }

  if (READ_PATTERNS.some((pattern) => pattern.test(normalized))) {
    return {
      class: "read",
      reason: "Read-only command is auto-allowed",
      requiresApproval: false,
    };
  }

  if (WRITE_PATTERNS.some((pattern) => pattern.test(normalized))) {
    const isCosmos = commandTargetsCosmosNative(normalized, cwd);
    const isEvmWrite = /^cast\s+send\b/i.test(normalized) || /^hyperlane\s+/i.test(normalized);

    if (isEvmWrite && !hasEnv("HYP_KEY")) {
      return {
        class: "blocked",
        reason: "Missing required env var HYP_KEY for EVM mutating command",
        remediation: "export HYP_KEY=0x...",
        requiresApproval: false,
      };
    }

    if (isCosmos && !hasEnv("HYP_KEY_COSMOSNATIVE")) {
      return {
        class: "blocked",
        reason: "Missing required env var HYP_KEY_COSMOSNATIVE for cosmosnative mutating command",
        remediation: "export HYP_KEY_COSMOSNATIVE=0x...",
        requiresApproval: false,
      };
    }

    return {
      class: "write",
      reason: "Mutating command requires explicit approval",
      requiresApproval: true,
    };
  }

  return {
    class: "blocked",
    reason: "Command is outside the v1 allowlist",
    remediation: "Use supported core/warp/relayer workflows or run manually",
    requiresApproval: false,
  };
}

export function evaluatePipeline(command: string, cwd: string): PolicyEvaluation {
  const segments = splitCommandSegments(command);
  if (!segments.length) {
    return evaluateCommand(command, cwd);
  }

  let hasWrite = false;

  for (const segment of segments) {
    const evaluation = evaluateCommand(segment, cwd);
    if (evaluation.class === "blocked") {
      return evaluation;
    }
    if (evaluation.class === "write") {
      hasWrite = true;
    }
  }

  if (hasWrite) {
    return {
      class: "write",
      reason: "Pipeline includes mutating command(s), approval required",
      requiresApproval: true,
    };
  }

  return {
    class: "read",
    reason: "Pipeline is read-only",
    requiresApproval: false,
  };
}
