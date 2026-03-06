#!/usr/bin/env node

import { resolve } from "node:path";
import { startServer } from "./server.js";

type Args = {
  _: string[];
  flags: Record<string, string | boolean | string[]>;
};

const COMMAND_HELP: Record<string, string> = {
  serve: `Hyperlane Ops Agent - serve

Usage:
  ops-agent serve [--port 8787] [--cwd /path]

Options:
  --port <number>   Port to bind (default: 8787)
  --cwd <path>      Working directory for repository operations

Examples:
  ops-agent serve
  ops-agent serve --port 8787 --cwd /Users/damiannolan/development/hyperlane-ops`,
  plan: `Hyperlane Ops Agent - plan

Usage:
  ops-agent plan "<goal>" [--json] [--host http://127.0.0.1:8787]

Options:
  --json            Output JSON
  --host <url>      Ops-agent server URL

Examples:
  ops-agent plan "create deployment plan for hyperlane core on sepolia"
  ops-agent plan "check relayer health" --host http://127.0.0.1:8787`,
  info: `Hyperlane Ops Agent - info

Usage:
  ops-agent info [--verify] [--json] [--host http://127.0.0.1:8787]

Options:
  --verify          Perform runtime Pi verification call
  --json            Output raw JSON
  --host <url>      Ops-agent server URL

Examples:
  ops-agent info
  ops-agent info --verify
  ops-agent info --verify --json`,
  approve: `Hyperlane Ops Agent - approve

Usage:
  ops-agent approve <planId> --all [--ttl 900] [--host http://127.0.0.1:8787]
  ops-agent approve <planId> --hash <hash> [--hash <hash2>] [--ttl 900] [--host ...]

Options:
  --all             Approve all non-blocked commands in the plan
  --hash <hash>     Approve one specific command hash (repeatable)
  --ttl <seconds>   Approval token TTL in seconds (default: 900)
  --host <url>      Ops-agent server URL

Examples:
  ops-agent approve <planId> --all
  ops-agent approve <planId> --hash <hash> --ttl 1200`,
  execute: `Hyperlane Ops Agent - execute

Usage:
  ops-agent execute <planId> --token <approvalToken> [--host http://127.0.0.1:8787]

Options:
  --token <token>   Approval token from approve command
  --host <url>      Ops-agent server URL

Examples:
  ops-agent execute <planId> --token <approvalToken>`,
  run: `Hyperlane Ops Agent - run

Usage:
  ops-agent run "<goal>" --read-only [--host http://127.0.0.1:8787]

Options:
  --read-only       Required for safe diagnostic run mode
  --host <url>      Ops-agent server URL

Examples:
  ops-agent run "diagnose relayer lag" --read-only`,
};

type InfoResponse = {
  model?: {
    provider?: string;
    model?: string;
  };
  credentials?: {
    openaiApiKeyPresent?: boolean;
    hypKeyPresent?: boolean;
    hypKeyCosmosnativePresent?: boolean;
    authFilePath?: string;
    authFilePresent?: boolean;
    authFileHasOpenAiApiKey?: boolean;
    authFileHasOpenAiCodexOAuth?: boolean;
    authFileParseError?: string;
  };
  runtime?: {
    cwd?: string;
    sessionDir?: string;
  };
  verification?: {
    requested?: boolean;
    ok?: boolean;
    provider?: string;
    model?: string;
    error?: string;
  };
};

function parseArgs(argv: string[]): Args {
  const args: Args = { _: [], flags: {} };

  for (let i = 0; i < argv.length; i += 1) {
    const token = argv[i];
    if (token === "-h") {
      args.flags.help = true;
      continue;
    }
    if (token.startsWith("--")) {
      const key = token.slice(2);
      const next = argv[i + 1];
      if (!next || next.startsWith("--")) {
        args.flags[key] = true;
      } else {
        const existing = args.flags[key];
        if (typeof existing === "string") {
          args.flags[key] = [existing, next];
        } else if (Array.isArray(existing)) {
          args.flags[key] = [...existing, next];
        } else {
          args.flags[key] = next;
        }
        i += 1;
      }
      continue;
    }

    args._.push(token);
  }

  return args;
}

function requireString(value: string | boolean | string[] | undefined, field: string): string {
  if (typeof value === "string" && value.length > 0) {
    return value;
  }
  throw new Error(`Missing required flag --${field}`);
}

async function postJson(baseUrl: string, path: string, body: unknown): Promise<unknown> {
  try {
    const response = await fetch(`${baseUrl}${path}`, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
      },
      body: JSON.stringify(body),
    });

    const text = await response.text();
    const parsed = text ? JSON.parse(text) : {};

    if (!response.ok) {
      throw new Error(parsed.error ?? `Request failed (${response.status})`);
    }

    return parsed;
  } catch (error) {
    if (error instanceof TypeError && error.message.includes("fetch failed")) {
      throw new Error(
        `Could not reach ops-agent at ${baseUrl}. Start the server first (for example: ops-agent serve), or pass --host <url>.`,
      );
    }
    throw error;
  }
}

async function getJson(baseUrl: string, path: string): Promise<unknown> {
  try {
    const response = await fetch(`${baseUrl}${path}`);
    const text = await response.text();
    const parsed = text ? JSON.parse(text) : {};

    if (!response.ok) {
      throw new Error(parsed.error ?? `Request failed (${response.status})`);
    }

    return parsed;
  } catch (error) {
    if (error instanceof TypeError && error.message.includes("fetch failed")) {
      throw new Error(
        `Could not reach ops-agent at ${baseUrl}. Start the server first (for example: ops-agent serve), or pass --host <url>.`,
      );
    }
    throw error;
  }
}

function printUsage(): void {
  // eslint-disable-next-line no-console
  console.log(`Hyperlane Ops Agent

Usage:
  ops-agent serve [--port 8787] [--cwd /path]
  ops-agent plan "<goal>" [--json] [--host http://127.0.0.1:8787]
  ops-agent info [--verify] [--json] [--host ...]
  ops-agent approve <planId> --all [--ttl 900] [--host ...]
  ops-agent approve <planId> --hash <hash> [--hash <hash2>] [--ttl 900] [--host ...]
  ops-agent execute <planId> --token <token> [--host ...]
  ops-agent run "<goal>" --read-only [--host ...]

Help:
  ops-agent help
  ops-agent help <command>
  ops-agent <command> --help`);
}

function printCommandUsage(command: string): boolean {
  const message = COMMAND_HELP[command];
  if (!message) {
    return false;
  }
  // eslint-disable-next-line no-console
  console.log(message);
  return true;
}

function wantsHelp(parsed: Args): boolean {
  return parsed.flags.help === true;
}

function yesNo(value: boolean | undefined): string {
  return value ? "yes" : "no";
}

function asDisplayValue(value: string | undefined): string {
  return value && value.trim().length ? value : "(not set)";
}

function renderInfoHuman(info: InfoResponse): string {
  const lines: string[] = [];
  const provider = info.model?.provider ?? "";
  const model = info.model?.model ?? "";
  const creds = info.credentials ?? {};
  const runtime = info.runtime ?? {};

  lines.push("Hyperlane Ops Agent Info");
  lines.push("");
  lines.push("Model");
  lines.push(`  Provider: ${asDisplayValue(provider)}`);
  lines.push(`  Model: ${asDisplayValue(model)}`);
  lines.push("");
  lines.push("Credentials");
  lines.push(`  OPENAI_API_KEY present: ${yesNo(creds.openaiApiKeyPresent)}`);
  lines.push(`  HYP_KEY present: ${yesNo(creds.hypKeyPresent)}`);
  lines.push(`  HYP_KEY_COSMOSNATIVE present: ${yesNo(creds.hypKeyCosmosnativePresent)}`);
  lines.push(`  Auth file path: ${asDisplayValue(creds.authFilePath)}`);
  lines.push(`  Auth file present: ${yesNo(creds.authFilePresent)}`);
  lines.push(`  Auth has OpenAI API key: ${yesNo(creds.authFileHasOpenAiApiKey)}`);
  lines.push(`  Auth has openai-codex OAuth: ${yesNo(creds.authFileHasOpenAiCodexOAuth)}`);
  if (creds.authFileParseError) {
    lines.push(`  Auth parse error: ${creds.authFileParseError}`);
  }
  lines.push("");
  lines.push("Runtime");
  lines.push(`  CWD: ${asDisplayValue(runtime.cwd)}`);
  lines.push(`  PI_SESSION_DIR: ${asDisplayValue(runtime.sessionDir)}`);

  if (info.verification?.requested) {
    lines.push("");
    lines.push("Verification");
    lines.push(`  Result: ${info.verification.ok ? "ok" : "failed"}`);
    lines.push(`  Provider: ${asDisplayValue(info.verification.provider)}`);
    lines.push(`  Model: ${asDisplayValue(info.verification.model)}`);
    if (info.verification.error) {
      lines.push(`  Error: ${info.verification.error}`);
    }
  }

  return `${lines.join("\n")}\n`;
}

async function main(): Promise<void> {
  const parsed = parseArgs(process.argv.slice(2));
  const [command] = parsed._;
  const host = typeof parsed.flags.host === "string" ? parsed.flags.host : "http://127.0.0.1:8787";

  if (!command) {
    printUsage();
    return;
  }

  if (command === "help" || command === "--help" || command === "-h") {
    const topic = parsed._[1];
    if (!topic) {
      printUsage();
      return;
    }
    if (!printCommandUsage(topic)) {
      throw new Error(`Unknown command '${topic}'. Run 'ops-agent help' for available commands.`);
    }
    return;
  }

  if (command === "serve") {
    if (wantsHelp(parsed)) {
      printCommandUsage("serve");
      return;
    }
    if (typeof parsed.flags.port === "string") {
      process.env.OPS_AGENT_PORT = parsed.flags.port;
    }
    if (typeof parsed.flags.cwd === "string") {
      process.env.OPS_AGENT_CWD = resolve(parsed.flags.cwd);
    }
    await startServer();
    return;
  }

  if (command === "plan") {
    if (wantsHelp(parsed)) {
      printCommandUsage("plan");
      return;
    }
    const goal = parsed._.slice(1).join(" ").trim();
    if (!goal) {
      throw new Error("Missing plan goal");
    }

    const response = await postJson(host, "/v1/plan", { goal });
    if (parsed.flags.json) {
      // eslint-disable-next-line no-console
      console.log(JSON.stringify(response, null, 2));
      return;
    }

    // eslint-disable-next-line no-console
    console.log(JSON.stringify(response, null, 2));
    return;
  }

  if (command === "info") {
    if (wantsHelp(parsed)) {
      printCommandUsage("info");
      return;
    }
    const verify = Boolean(parsed.flags.verify);
    const path = verify ? "/v1/info?verify=1" : "/v1/info";
    const response = (await getJson(host, path)) as InfoResponse;
    if (parsed.flags.json) {
      // eslint-disable-next-line no-console
      console.log(JSON.stringify(response, null, 2));
      return;
    }
    // eslint-disable-next-line no-console
    process.stdout.write(renderInfoHuman(response));
    return;
  }

  if (command === "approve") {
    if (wantsHelp(parsed)) {
      printCommandUsage("approve");
      return;
    }
    const planId = parsed._[1];
    if (!planId) {
      throw new Error("Missing planId");
    }

    const ttlSeconds = Number(parsed.flags.ttl ?? 900);

    if (parsed.flags.all) {
      const plan = (await getJson(host, `/v1/plans/${planId}`)) as {
        commands?: Array<{ hash: string; class: string }>;
      };
      const hashes = plan.commands?.filter((item) => item.class !== "blocked").map((item) => item.hash) ?? [];
      const response = await postJson(host, "/v1/approve", {
        planId,
        commandHashes: hashes,
        ttlSeconds,
      });
      // eslint-disable-next-line no-console
      console.log(JSON.stringify(response, null, 2));
      return;
    }

    const hashFlag = parsed.flags.hash;
    const hashes = Array.isArray(hashFlag)
      ? hashFlag.filter((item): item is string => typeof item === "string")
      : typeof hashFlag === "string"
        ? [hashFlag]
        : [];

    if (!hashes.length) {
      throw new Error("Use --all or provide at least one --hash value");
    }

    const response = await postJson(host, "/v1/approve", {
      planId,
      commandHashes: hashes,
      ttlSeconds,
    });
    // eslint-disable-next-line no-console
    console.log(JSON.stringify(response, null, 2));
    return;
  }

  if (command === "execute") {
    if (wantsHelp(parsed)) {
      printCommandUsage("execute");
      return;
    }
    const planId = parsed._[1];
    if (!planId) {
      throw new Error("Missing planId");
    }

    const token = requireString(parsed.flags.token, "token");
    const response = await postJson(host, "/v1/execute", {
      planId,
      approvalToken: token,
    });
    // eslint-disable-next-line no-console
    console.log(JSON.stringify(response, null, 2));
    return;
  }

  if (command === "run") {
    if (wantsHelp(parsed)) {
      printCommandUsage("run");
      return;
    }
    const goal = parsed._.slice(1).join(" ").trim();
    if (!goal) {
      throw new Error("Missing run goal");
    }

    const plan = (await postJson(host, "/v1/plan", { goal })) as { planId: string };
    const response = await postJson(host, "/v1/execute", {
      planId: plan.planId,
      readOnly: true,
    });
    // eslint-disable-next-line no-console
    console.log(JSON.stringify(response, null, 2));
    return;
  }

  throw new Error(`Unknown command '${command}'. Run 'ops-agent help' for available commands.`);
}

void main().catch((error) => {
  // eslint-disable-next-line no-console
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
});
