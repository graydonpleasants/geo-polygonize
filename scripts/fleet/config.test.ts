import { expect, test, describe } from "bun:test";
import { validateFleetId } from "./config";

describe("validateFleetId", () => {
  test("accepts valid alphanumeric IDs with underscores", () => {
    expect(validateFleetId("2026_02_26")).toBe("2026_02_26");
    expect(validateFleetId("fleet_id_123")).toBe("fleet_id_123");
    expect(validateFleetId("ABC_123")).toBe("ABC_123");
  });

  test("rejects IDs with hyphens", () => {
    expect(() => validateFleetId("fleet-id-123")).toThrow("Only alphanumeric characters and underscores are allowed");
  });

  test("rejects IDs with path traversal characters", () => {
    expect(() => validateFleetId("../etc")).toThrow("Only alphanumeric characters and underscores are allowed");
    expect(() => validateFleetId("../../root")).toThrow("Only alphanumeric characters and underscores are allowed");
    expect(() => validateFleetId("./local")).toThrow("Only alphanumeric characters and underscores are allowed");
  });

  test("rejects IDs with special characters", () => {
    expect(() => validateFleetId("bad$char")).toThrow("Only alphanumeric characters and underscores are allowed");
    expect(() => validateFleetId("fleet id")).toThrow("Only alphanumeric characters and underscores are allowed");
    expect(() => validateFleetId("fleet.id")).toThrow("Only alphanumeric characters and underscores are allowed");
  });
});
