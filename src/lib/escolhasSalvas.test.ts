import { beforeEach, describe, expect, it } from "vitest";
import { contaDoAgente, lembrarAjustes, lembrarConta, opcoesDeModelo, ajustesDoAgente } from "./escolhasSalvas";

describe("escolhasSalvas", () => {
  beforeEach(() => localStorage.clear());

  it("lembra conta e ajustes por agente", () => {
    lembrarConta("claude", "conta-a");
    lembrarAjustes("codex", { model: "m1", effort: "high" });
    expect(contaDoAgente("claude")).toBe("conta-a");
    expect(contaDoAgente("codex")).toBeNull();
    expect(ajustesDoAgente("codex")).toEqual({ model: "m1", effort: "high" });
  });

  it("monta as opções na sintaxe de cada CLI e ignora valores estranhos", () => {
    expect(opcoesDeModelo("claude", { model: "opus", effort: "high" })).toBe(" --model opus --effort high");
    expect(opcoesDeModelo("codex", { model: "m1", effort: "low" })).toBe(" -m m1 -c model_reasoning_effort=low");
    expect(opcoesDeModelo("antigravity", { model: "fam", effort: null })).toBe(" --model fam");
    expect(opcoesDeModelo("cursor", { model: "x", effort: null })).toBe("");
    expect(opcoesDeModelo("claude", { model: "a; rm -rf ~", effort: null })).toBe("");
  });
});
