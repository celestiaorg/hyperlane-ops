import { spawn, type ChildProcess } from "node:child_process";
import { createInterface, type Interface } from "node:readline";

interface RpcResponse {
  id?: string;
  type: "response";
  command: string;
  success: boolean;
  data?: unknown;
  error?: string;
}

interface PendingRequest {
  resolve: (value: RpcResponse) => void;
  reject: (error: Error) => void;
  timeout: NodeJS.Timeout;
}

export interface PiRpcEvent {
  type: string;
  [key: string]: unknown;
}

export interface PiRpcOptions {
  cwd: string;
  provider?: string;
  model?: string;
  sessionDir?: string;
  timeoutMs?: number;
}

const DEFAULT_TIMEOUT_MS = 120_000;

export class PiRpcClient {
  private process: ChildProcess | null = null;
  private rl: Interface | null = null;
  private stderr = "";
  private requestId = 0;
  private pending = new Map<string, PendingRequest>();
  private eventListeners = new Set<(event: PiRpcEvent) => void>();

  constructor(private readonly options: PiRpcOptions) {}

  async start(): Promise<void> {
    if (this.process) {
      throw new Error("Pi RPC client already started");
    }

    const args = ["--mode", "rpc", "--no-session"];

    if (this.options.provider) {
      args.push("--provider", this.options.provider);
    }
    if (this.options.model) {
      args.push("--model", this.options.model);
    }
    if (this.options.sessionDir) {
      args.push("--session-dir", this.options.sessionDir);
    }

    const command = process.platform === "win32" ? "pi.cmd" : "pi";

    this.process = spawn(command, args, {
      cwd: this.options.cwd,
      env: process.env,
      stdio: ["pipe", "pipe", "pipe"],
    });

    this.process.stderr?.on("data", (chunk) => {
      this.stderr += chunk.toString();
    });

    this.rl = createInterface({
      input: this.process.stdout!,
      terminal: false,
    });

    this.rl.on("line", (line) => {
      this.onLine(line);
    });

    this.process.on("exit", (code) => {
      const error = new Error(`Pi RPC exited with code ${code}. Stderr: ${this.stderr}`);
      for (const req of this.pending.values()) {
        clearTimeout(req.timeout);
        req.reject(error);
      }
      this.pending.clear();
    });

    await new Promise((resolve) => setTimeout(resolve, 100));

    if (this.process.exitCode !== null) {
      throw new Error(`Pi RPC exited immediately with code ${this.process.exitCode}. Stderr: ${this.stderr}`);
    }
  }

  async stop(): Promise<void> {
    if (!this.process) {
      return;
    }

    this.rl?.close();
    this.process.kill("SIGTERM");

    await new Promise<void>((resolve) => {
      const timeout = setTimeout(() => {
        this.process?.kill("SIGKILL");
        resolve();
      }, 1000);

      this.process?.once("exit", () => {
        clearTimeout(timeout);
        resolve();
      });
    });

    this.process = null;
    this.rl = null;
    this.stderr = "";
  }

  onEvent(listener: (event: PiRpcEvent) => void): () => void {
    this.eventListeners.add(listener);
    return () => this.eventListeners.delete(listener);
  }

  async prompt(message: string): Promise<void> {
    await this.send({ type: "prompt", message });
  }

  async getLastAssistantText(): Promise<string | null> {
    const response = await this.send({ type: "get_last_assistant_text" });
    return (response.data as { text: string | null } | undefined)?.text ?? null;
  }

  async collectEventsUntilIdle(timeoutMs = DEFAULT_TIMEOUT_MS): Promise<PiRpcEvent[]> {
    return new Promise((resolve, reject) => {
      const events: PiRpcEvent[] = [];
      const timer = setTimeout(() => {
        unsubscribe();
        reject(new Error(`Timed out waiting for agent_end. Stderr: ${this.stderr}`));
      }, timeoutMs);

      const unsubscribe = this.onEvent((event) => {
        events.push(event);
        if (event.type === "agent_end") {
          clearTimeout(timer);
          unsubscribe();
          resolve(events);
        }
      });
    });
  }

  async promptAndWait(message: string, timeoutMs = DEFAULT_TIMEOUT_MS): Promise<PiRpcEvent[]> {
    const eventsPromise = this.collectEventsUntilIdle(timeoutMs);
    await this.prompt(message);
    return eventsPromise;
  }

  private onLine(line: string): void {
    let parsed: unknown;
    try {
      parsed = JSON.parse(line);
    } catch {
      return;
    }

    const asRecord = parsed as Record<string, unknown>;
    if (asRecord.type === "response") {
      const id = asRecord.id as string | undefined;
      if (id && this.pending.has(id)) {
        const pending = this.pending.get(id)!;
        this.pending.delete(id);
        clearTimeout(pending.timeout);

        const response = parsed as RpcResponse;
        if (response.success) {
          pending.resolve(response);
        } else {
          pending.reject(new Error(response.error ?? "Unknown RPC error"));
        }
      }
      return;
    }

    const event = parsed as PiRpcEvent;
    for (const listener of this.eventListeners) {
      listener(event);
    }
  }

  private async send(command: Record<string, unknown>): Promise<RpcResponse> {
    if (!this.process?.stdin) {
      throw new Error("Pi RPC client is not started");
    }

    const id = `req_${++this.requestId}`;
    const payload = { ...command, id };

    const timeoutMs = this.options.timeoutMs ?? DEFAULT_TIMEOUT_MS;

    const responsePromise = new Promise<RpcResponse>((resolve, reject) => {
      const timeout = setTimeout(() => {
        this.pending.delete(id);
        reject(new Error(`RPC command timed out: ${String(command.type)}. Stderr: ${this.stderr}`));
      }, timeoutMs);

      this.pending.set(id, {
        resolve,
        reject,
        timeout,
      });
    });

    this.process.stdin.write(`${JSON.stringify(payload)}\n`);
    return responsePromise;
  }
}
