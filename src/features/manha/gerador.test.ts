import { describe, expect, it } from "vitest";
import { criarRng, gerarQuestao, semente, sortearTemas, TEMAS_GERADOS, type Nivel } from "./gerador";

describe("gerador de questões", () => {
  it("toda questão tem 4 opções distintas, a certa entre elas e nada de NaN", () => {
    for (const tema of TEMAS_GERADOS) {
      for (const nivel of [1, 2, 3] as Nivel[]) {
        for (let i = 0; i < 300; i++) {
          const q = gerarQuestao(tema, nivel, criarRng(semente(`${tema}-${nivel}-${i}`)), "q");
          const contexto = `${tema}/${nivel}/${i}: ${q.enunciado} → ${q.opcoes.join(" | ")}`;
          expect(q.opcoes, contexto).toHaveLength(4);
          expect(new Set(q.opcoes).size, contexto).toBe(4);
          expect(q.correta, contexto).toBeGreaterThanOrEqual(0);
          expect(`${q.enunciado}${q.opcoes.join("")}${q.explicacao}`, contexto).not.toMatch(/NaN|Infinity|undefined/);
        }
      }
    }
  });

  it("mesma semente, mesmas questões (reabrir o app no meio não troca as perguntas)", () => {
    const a = gerarQuestao("equacoes", 2, criarRng(semente("2026-10-21")), "x");
    const b = gerarQuestao("equacoes", 2, criarRng(semente("2026-10-21")), "x");
    expect(a).toEqual(b);
  });

  it("distribui a quantidade entre os temas", () => {
    const temas = sortearTemas(["a", "b"], 5, criarRng(1));
    expect(temas).toHaveLength(5);
    expect(new Set(temas)).toEqual(new Set(["a", "b"]));
    expect(sortearTemas([], 5, criarRng(1))).toEqual([]);
  });
});
