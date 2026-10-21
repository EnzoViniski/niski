import { describe, expect, it } from "vitest";
import { deslocamentos } from "../features/projects/animarLateral";

/* Fork (lateral fluida): o que desliza e o que entra quando a lista da lateral muda. */
describe("deslocamentos da lateral", () => {
  it("linha que mudou de lugar desliza da posição antiga; parada não anima; nova entra", () => {
    const antes = new Map([["a", { topo: 0 }], ["b", { topo: 30 }], ["c", { topo: 60 }]]);
    const depois = new Map([["b", { topo: 0 }], ["a", { topo: 30 }], ["c", { topo: 60 }], ["d", { topo: 90 }]]);
    const r = deslocamentos(antes, depois);
    expect(r.get("a")).toBe(-30);
    expect(r.get("b")).toBe(30);
    expect(r.has("c")).toBe(false);
    expect(r.get("d")).toBeNull();
  });

  it("diferença menor que 1 px não anima (arredondamento)", () => {
    expect(deslocamentos(new Map([["a", { topo: 10 }]]), new Map([["a", { topo: 10.4 }]])).size).toBe(0);
  });
});
