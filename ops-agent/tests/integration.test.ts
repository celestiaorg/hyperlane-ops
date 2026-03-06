import { mkdtempSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { afterEach, describe, expect, test } from "vitest";
import type { DecisionEngine } from "../src/types.js";
import { PlanStore } from "../src/plan-store.js";
import { createMetrics } from "../src/metrics.js";
import { OpsOrchestrator } from "../src/orchestrator.js";

const ORIGINAL_ENV = { ...process.env };

afterEach(() => {
  process.env = { ...ORIGINAL_ENV };
});

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function waitForCompletion(orchestrator: OpsOrchestrator, runId: string): Promise<void> {
  for (let i = 0; i < 100; i += 1) {
    const run = orchestrator.getRun(runId);
    if (run && (run.status === "completed" || run.status === "failed")) {
      return;
    }
    await sleep(10);
  }
  throw new Error("Run did not complete in time");
}

class MockDecisionEngine implements DecisionEngine {
  async createPlan() {
    return {
      summary: "mock summary",
      commands: ["docker compose ps", "docker compose restart relayer"],
    };
  }
}

describe("orchestrator integration", () => {
  test("read-only run skips mutating command", async () => {
    const dir = mkdtempSync(join(tmpdir(), "ops-agent-int-"));
    const store = new PlanStore(join(dir, "store.json"));
    const metrics = createMetrics();
    const orchestrator = new OpsOrchestrator({
      baseCwd: process.cwd(),
      decisionEngine: new MockDecisionEngine(),
      store,
      metrics,
      executor: async () => ({ exitCode: 0, stdout: "ok", stderr: "" }),
    });

    const { plan } = await orchestrator.createPlan("check relayer");
    const execution = await orchestrator.execute(plan.id, { readOnly: true });

    await waitForCompletion(orchestrator, execution.runId);

    const run = orchestrator.getRun(execution.runId);
    expect(run?.status).toBe("completed");
    expect(run?.commands.some((command) => command.status === "skipped")).toBe(true);
  });

  test("approved run executes mutating command", async () => {
    const dir = mkdtempSync(join(tmpdir(), "ops-agent-int-"));
    const store = new PlanStore(join(dir, "store.json"));
    const metrics = createMetrics();
    const orchestrator = new OpsOrchestrator({
      baseCwd: process.cwd(),
      decisionEngine: new MockDecisionEngine(),
      store,
      metrics,
      executor: async () => ({ exitCode: 0, stdout: "ok", stderr: "" }),
    });

    const { plan } = await orchestrator.createPlan("restart relayer");
    const approval = orchestrator.approve(
      plan.id,
      plan.commands.filter((command) => command.class !== "blocked").map((command) => command.hash),
      600,
    );

    const execution = await orchestrator.execute(plan.id, { approvalToken: approval.token });
    await waitForCompletion(orchestrator, execution.runId);

    const run = orchestrator.getRun(execution.runId);
    expect(run?.status).toBe("completed");
    expect(run?.commands.some((command) => command.class === "write" && command.status === "executed")).toBe(true);
  });
});
