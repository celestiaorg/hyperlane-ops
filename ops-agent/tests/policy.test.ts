import { afterEach, describe, expect, test } from "vitest";
import { evaluatePipeline } from "../src/policy.js";

const ORIGINAL_ENV = { ...process.env };

afterEach(() => {
  process.env = { ...ORIGINAL_ENV };
});

describe("policy", () => {
  test("allows read-only relayer checks", () => {
    const result = evaluatePipeline("docker compose ps", process.cwd());
    expect(result.class).toBe("read");
  });

  test("blocks hyperlane mutation missing --registry .", () => {
    process.env.HYP_KEY = "0xabc";
    const result = evaluatePipeline("hyperlane core deploy --chain sepolia --config configs/sepolia-core.yaml", process.cwd());
    expect(result.class).toBe("blocked");
    expect(result.reason).toContain("--registry .");
  });

  test("requires HYP_KEY for EVM mutating command", () => {
    delete process.env.HYP_KEY;
    const result = evaluatePipeline("cast send 0xabc \"foo()\"", process.cwd());
    expect(result.class).toBe("blocked");
    expect(result.reason).toContain("HYP_KEY");
  });

  test("marks write command when env and registry checks pass", () => {
    process.env.HYP_KEY = "0xabc";
    const result = evaluatePipeline(
      "hyperlane core deploy --chain sepolia --config configs/sepolia-core.yaml --registry .",
      process.cwd(),
    );
    expect(result.class).toBe("write");
  });
});
