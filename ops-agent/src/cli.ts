#!/usr/bin/env node

type Args = {
  _: string[];
  flags: Record<string, string | boolean | string[]>;
};

function parseArgs(argv: string[]): Args {
  const args: Args = { _: [], flags: {} };

  for (let i = 0; i < argv.length; i += 1) {
    const token = argv[i];
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
}

async function getJson(baseUrl: string, path: string): Promise<unknown> {
  const response = await fetch(`${baseUrl}${path}`);
  const text = await response.text();
  const parsed = text ? JSON.parse(text) : {};

  if (!response.ok) {
    throw new Error(parsed.error ?? `Request failed (${response.status})`);
  }

  return parsed;
}

function printUsage(): void {
  // eslint-disable-next-line no-console
  console.log(`Usage:
  ops-agent plan "<goal>" [--json] [--host http://127.0.0.1:8787]
  ops-agent approve <planId> --all [--ttl 900] [--host ...]
  ops-agent approve <planId> --hash <hash> [--hash <hash2>] [--ttl 900] [--host ...]
  ops-agent execute <planId> --token <token> [--host ...]
  ops-agent run "<goal>" --read-only [--host ...]`);
}

async function main(): Promise<void> {
  const parsed = parseArgs(process.argv.slice(2));
  const [command] = parsed._;
  const host = typeof parsed.flags.host === "string" ? parsed.flags.host : "http://127.0.0.1:8787";

  if (!command) {
    printUsage();
    process.exitCode = 1;
    return;
  }

  if (command === "plan") {
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

  if (command === "approve") {
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

  printUsage();
  process.exitCode = 1;
}

void main().catch((error) => {
  // eslint-disable-next-line no-console
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
});
