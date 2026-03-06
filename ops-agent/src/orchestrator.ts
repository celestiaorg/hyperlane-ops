import { resolve } from "node:path";
import { hashCommand } from "./command-parser.js";
import { executeShellCommand, type CommandExecutionResult } from "./executor.js";
import type { AgentMetrics } from "./metrics.js";
import { evaluatePipeline } from "./policy.js";
import { makeRunEvent } from "./transcript.js";
import type { DecisionEngine, PlanRecord } from "./types.js";
import { FallbackDecisionEngine } from "./decision-engine.js";
import { PlanStore } from "./plan-store.js";

export interface ExecuteRunOptions {
  approvalToken?: string;
  readOnly?: boolean;
}

export interface OrchestratorOptions {
  baseCwd: string;
  decisionEngine: DecisionEngine;
  store: PlanStore;
  metrics: AgentMetrics;
  executor?: (command: string, cwd: string) => Promise<CommandExecutionResult>;
}

export class OpsOrchestrator {
  private readonly executor: (command: string, cwd: string) => Promise<CommandExecutionResult>;

  constructor(private readonly options: OrchestratorOptions) {
    this.executor = options.executor ?? executeShellCommand;
  }

  async createPlan(goal: string, context?: string, cwd?: string) {
    const resolvedCwd = resolve(cwd ? cwd : this.options.baseCwd);

    const decisionPlan = await this.makePlan(goal, context, resolvedCwd);

    const commands = decisionPlan.commands.map((command) => {
      const evaluation = evaluatePipeline(command, resolvedCwd);
      return {
        hash: hashCommand(command),
        command,
        class: evaluation.class,
        reason: evaluation.reason,
        remediation: evaluation.remediation,
      };
    });

    const plan = this.options.store.createPlan({
      goal,
      context,
      cwd: resolvedCwd,
      summary: decisionPlan.summary,
      commands,
    });

    this.options.metrics.plansCreatedTotal.inc();

    return {
      plan,
      rawResponse: decisionPlan.rawResponse,
    };
  }

  getPlan(planId: string): PlanRecord | undefined {
    return this.options.store.getPlan(planId);
  }

  approve(planId: string, commandHashes: string[], ttlSeconds = 900) {
    const plan = this.options.store.getPlan(planId);
    if (!plan) {
      throw new Error("Plan not found");
    }

    const uniqueHashes = [...new Set(commandHashes)];
    const invalidHashes = uniqueHashes.filter(
      (hash) => !plan.commands.some((command) => command.hash === hash && command.class !== "blocked"),
    );

    if (invalidHashes.length > 0) {
      throw new Error(`Approval contains invalid hashes: ${invalidHashes.join(",")}`);
    }

    return this.options.store.createApproval(planId, uniqueHashes, ttlSeconds);
  }

  async execute(planId: string, executeOptions: ExecuteRunOptions) {
    const plan = this.options.store.getPlan(planId);
    if (!plan) {
      throw new Error("Plan not found");
    }

    const requiresWriteApproval = plan.commands.some((command) => command.class === "write");
    let approvedHashes = new Set<string>();

    if (!executeOptions.readOnly && requiresWriteApproval) {
      if (!executeOptions.approvalToken) {
        throw new Error("approvalToken is required for plans with mutating commands");
      }
      const approval = this.options.store.consumeApproval(plan.id, executeOptions.approvalToken);
      approvedHashes = new Set(approval.commandHashes);
    }

    const run = this.options.store.createRun(plan.id);
    this.options.store.appendRunEvent(run.id, makeRunEvent("status", "Run queued"));

    void this.executeRun(run.id, planId, approvedHashes, Boolean(executeOptions.readOnly));

    return {
      runId: run.id,
      eventsEndpoint: `/v1/runs/${run.id}/events`,
    };
  }

  getRun(runId: string) {
    return this.options.store.getRun(runId);
  }

  private async executeRun(runId: string, planId: string, approvedHashes: Set<string>, readOnly: boolean) {
    const plan = this.options.store.getPlan(planId);
    if (!plan) {
      this.options.store.appendRunEvent(runId, makeRunEvent("error", "Plan not found for run"));
      this.options.store.setRunStatus(runId, "failed");
      return;
    }

    try {
      this.options.store.setRunStatus(runId, "running");
      this.options.store.appendRunEvent(runId, makeRunEvent("status", "Run started"));

      for (const command of plan.commands) {
        const freshPolicy = evaluatePipeline(command.command, plan.cwd);
        const policyClass = freshPolicy.class;

        if (readOnly && policyClass !== "read") {
          this.options.store.updateRun(runId, (run) => {
            run.commands.push({
              hash: command.hash,
              command: command.command,
              class: policyClass,
              status: "skipped",
              reason: "Skipped because readOnly execution was requested",
            });
          });
          continue;
        }

        if (policyClass === "blocked") {
          this.options.metrics.policyViolationTotal.inc();
          if (command.class === "write") {
            this.options.metrics.writesBlockedTotal.inc();
          }
          this.options.store.updateRun(runId, (run) => {
            run.commands.push({
              hash: command.hash,
              command: command.command,
              class: policyClass,
              status: "blocked",
              reason: freshPolicy.reason,
            });
          });
          this.options.store.appendRunEvent(
            runId,
            makeRunEvent("error", `Blocked command: ${freshPolicy.reason}`, command.hash),
          );
          continue;
        }

        if (policyClass === "write" && !approvedHashes.has(command.hash)) {
          this.options.metrics.writesBlockedTotal.inc();
          this.options.store.updateRun(runId, (run) => {
            run.commands.push({
              hash: command.hash,
              command: command.command,
              class: policyClass,
              status: "blocked",
              reason: "Command hash not approved",
            });
          });
          this.options.store.appendRunEvent(
            runId,
            makeRunEvent("error", "Write command blocked: missing approval hash", command.hash),
          );
          continue;
        }

        this.options.store.appendRunEvent(runId, makeRunEvent("command", command.command, command.hash));
        const result = await this.executor(command.command, plan.cwd);

        const status = result.exitCode === 0 ? "executed" : "failed";
        this.options.store.updateRun(runId, (run) => {
          run.commands.push({
            hash: command.hash,
            command: command.command,
            class: policyClass,
            status,
            exitCode: result.exitCode,
            reason: status === "failed" ? "Command exited with non-zero status" : undefined,
          });
        });

        if (policyClass === "write" && status === "executed") {
          this.options.metrics.writesExecutedTotal.inc();
        }

        if (result.stdout.trim()) {
          this.options.store.appendRunEvent(runId, makeRunEvent("output", result.stdout, command.hash));
        }
        if (result.stderr.trim()) {
          this.options.store.appendRunEvent(runId, makeRunEvent("error", result.stderr, command.hash));
        }

        if (status === "failed") {
          this.options.metrics.runFailuresTotal.inc();
          this.options.store.setRunStatus(runId, "failed");
          this.options.store.appendRunEvent(runId, makeRunEvent("status", "Run failed"));
          return;
        }
      }

      this.options.store.setRunStatus(runId, "completed");
      this.options.store.appendRunEvent(runId, makeRunEvent("status", "Run completed"));
    } catch (error) {
      this.options.metrics.runFailuresTotal.inc();
      this.options.store.appendRunEvent(
        runId,
        makeRunEvent("error", error instanceof Error ? error.message : String(error)),
      );
      this.options.store.setRunStatus(runId, "failed");
    }
  }

  private async makePlan(goal: string, context: string | undefined, cwd: string) {
    try {
      return await this.options.decisionEngine.createPlan(goal, context, cwd);
    } catch (error) {
      const fallback = new FallbackDecisionEngine();
      const fallbackPlan = await fallback.createPlan(goal, context, cwd);
      return {
        ...fallbackPlan,
        rawResponse: `Fallback used after engine error: ${error instanceof Error ? error.message : String(error)}`,
      };
    }
  }
}
