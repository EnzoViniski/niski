// Fork (troca-de-contexto): a "última olhada" sobrevive, nunca anda para trás e tolera lixo.
import { beforeEach, describe, expect, it, vi } from "vitest";
import { CHAVE_DA_OLHADA, lerOlhada, lerOlhadas, marcarOlhada, ouvirOlhadas, versaoDasOlhadas } from "./ultimaOlhada";
import { CHAVES_ESPELHADAS } from "../../lib/espelhoDaInterface";

beforeEach(() => localStorage.clear());

describe("ultimaOlhada", () => {
  it("grava, lê e avisa quem ouve", () => {
    const ouvinte = vi.fn();
    const parar = ouvirOlhadas(ouvinte);
    const antes = versaoDasOlhadas();
    marcarOlhada("c1", { n: 4, em: 1000, tom: "pronto" });
    expect(lerOlhada("c1")).toEqual({ n: 4, em: 1000, tom: "pronto" });
    expect(ouvinte).toHaveBeenCalledTimes(1);
    expect(versaoDasOlhadas()).toBe(antes + 1);
    marcarOlhada("c1", { n: 4, em: 1000, tom: "pronto" });
    expect(ouvinte).toHaveBeenCalledTimes(1); // nada mudou, nada grava
    parar();
  });

  it("nunca anda para trás no número nem na hora", () => {
    marcarOlhada("c1", { n: 10, em: 5000 });
    marcarOlhada("c1", { n: 3, em: 2000, tom: "erro" });
    expect(lerOlhada("c1")).toEqual({ n: 10, em: 5000, tom: "erro" });
  });

  it("formato estranho ou versão desconhecida vira 'nunca olhou'", () => {
    localStorage.setItem(CHAVE_DA_OLHADA, "{quebrado");
    expect(lerOlhadas()).toEqual({});
    localStorage.setItem(CHAVE_DA_OLHADA, JSON.stringify({ v: 2, conversas: { c1: { n: 1, em: 1 } } }));
    expect(lerOlhada("c1")).toBeNull();
    localStorage.setItem(CHAVE_DA_OLHADA, JSON.stringify({ v: 1, conversas: { c1: { n: "x", em: 1 }, c2: { n: 2, em: 9, tom: "zzz" } } }));
    expect(lerOlhadas()).toEqual({ c2: { n: 2, em: 9, tom: null } });
  });

  it("guarda só as mais recentes", () => {
    for (let i = 0; i < 405; i += 1) marcarOlhada(`c${i}`, { n: 1, em: i + 1 });
    const todas = lerOlhadas();
    expect(Object.keys(todas)).toHaveLength(400);
    expect(todas.c0).toBeUndefined();
    expect(todas.c404).toBeDefined();
  });

  it("vai para o espelho em disco (sobrevive a fechar e atualizar)", () => {
    expect(CHAVES_ESPELHADAS).toContain(CHAVE_DA_OLHADA);
  });
});
