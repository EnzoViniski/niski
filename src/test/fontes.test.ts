import { describe, expect, it } from "vitest";
import { FONTE_MONO } from "../lib/fontes";

describe("fonte dos terminais", () => {
  it("tem uma monoespaçada do macOS antes do genérico (que no WebKit vira Courier)", () => {
    const partes = FONTE_MONO.split(",").map((parte) => parte.trim().replace(/"/g, ""));
    expect(partes.indexOf("Menlo")).toBeGreaterThan(-1);
    expect(partes.indexOf("Menlo")).toBeLessThan(partes.indexOf("monospace"));
    expect(partes.at(-1)).toBe("monospace");
  });
});
