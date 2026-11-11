import { describe, expect, it } from "vitest";
import { bounds, hit, move, nextStep, norm, parseDoc, type Item } from "./annotate";

const rect: Item = { id: "r", type: "rect", color: "#f00", width: 4, x1: 50, y1: 50, x2: 10, y2: 10 };
const step: Item = { id: "s", type: "step", color: "#f00", width: 4, x: 100, y: 100, n: 2 };

describe("annotations", () => {
  it("normalises rectangles", () => expect(norm(50, 50, 10, 10)).toEqual({ x: 10, y: 10, w: 40, h: 40 }));
  it("hit-tests the topmost item", () => {
    expect(hit([rect, step], 30, 30)?.id).toBe("r");
    expect(hit([rect, step], 100, 100)?.id).toBe("s");
    expect(hit([rect], 300, 300)).toBeNull();
  });
  it("moves items", () => expect(bounds(move(rect, 5, 5))).toEqual({ x: 15, y: 15, w: 40, h: 40 }));
  it("numbers steps", () => expect(nextStep([rect, step])).toBe(3));
  it("parses stored documents safely", () => {
    expect(parseDoc(null).items).toEqual([]);
    expect(parseDoc("{bad").items).toEqual([]);
    expect(parseDoc(JSON.stringify({ items: [rect], crop: { x: 1, y: 2, w: 3, h: 4 } })).crop).toEqual({ x: 1, y: 2, w: 3, h: 4 });
  });
});
