import { describe, expect, it } from "vitest";
import { PAGE_IDS, useNav } from "./nav";

describe("navigation store", () => {
  it("switches pages and ignores missing storage", () => {
    expect(PAGE_IDS[0]).toBe("home");
    useNav.getState().go("recordings");
    expect(useNav.getState().page).toBe("recordings");
    useNav.getState().go("home");
    expect(useNav.getState().page).toBe("home");
  });
});
