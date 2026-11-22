import { describe, expect, it } from "vitest";
import { clampIndex, moveIndex } from "./listNav";

describe("listNav", () => {
  it("clamps into the list and returns -1 for an empty list", () => {
    expect(clampIndex(5, 3)).toBe(2);
    expect(clampIndex(-4, 3)).toBe(0);
    expect(clampIndex(1, 3)).toBe(1);
    expect(clampIndex(0, 0)).toBe(-1);
  });

  it("moves with arrows without wrapping", () => {
    expect(moveIndex(0, 1, 5)).toBe(1);
    expect(moveIndex(4, 1, 5)).toBe(4); // انتهای فهرست
    expect(moveIndex(0, -1, 5)).toBe(0); // ابتدای فهرست
    expect(moveIndex(2, 8, 5)).toBe(4); // PageDown
    expect(moveIndex(2, -8, 5)).toBe(0); // PageUp
  });

  it("starts from the top or bottom when nothing is active", () => {
    expect(moveIndex(-1, 1, 5)).toBe(0);
    expect(moveIndex(-1, -1, 5)).toBe(4);
    expect(moveIndex(3, 1, 0)).toBe(-1);
  });
});
