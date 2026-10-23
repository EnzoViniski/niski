import { describe, expect, it } from "vitest";
import {
  ajustarCarga, ajustarRepeticoes, dataCurta, lerEstadoDaTela, lerLinhaDeExercicio, somarDias, ultimaVez, valoresIniciais,
} from "./dominio";

const serie = (exercicio: string, carga_kg: number, repeticoes: number) => ({ exercicio, carga_kg, repeticoes, obs: "", em: "" });

describe("estado salvo da tela", () => {
  it("lê o formato atual e tolera valor antigo, quebrado ou desconhecido", () => {
    expect(lerEstadoDaTela(JSON.stringify({ aba: "treino", conversaAberta: true, ficha: "fa" }))).toEqual({ aba: "treino", conversaAberta: true, ficha: "fa" });
    expect(lerEstadoDaTela(null)).toEqual({ aba: "hoje", conversaAberta: false, ficha: "" });
    expect(lerEstadoDaTela("{quebrado")).toEqual({ aba: "hoje", conversaAberta: false, ficha: "" });
    // Aba que não existe mais e campos de tipo errado caem no padrão, um a um.
    expect(lerEstadoDaTela(JSON.stringify({ aba: "habitos", conversaAberta: "sim", ficha: 3, extra: 1 }))).toEqual({ aba: "hoje", conversaAberta: false, ficha: "" });
  });
});

describe("registro rápido de série", () => {
  it("carga anda de 2,5 em 2,5 sem passar de 0 e 1000; repetições inteiras", () => {
    expect(ajustarCarga(60, 1)).toBe(62.5);
    expect(ajustarCarga(1, -1)).toBe(0);
    expect(ajustarCarga(999, 2)).toBe(1000);
    expect(ajustarRepeticoes(8, -10)).toBe(0);
    expect(ajustarRepeticoes(8, 1)).toBe(9);
  });

  it("começa pela última série de hoje, senão a última vez, senão a ficha", () => {
    expect(valoresIniciais([serie("Supino", 62.5, 8)], [serie("Supino", 60, 10)], 50, "12")).toEqual({ carga: 62.5, reps: 8 });
    expect(valoresIniciais([], [serie("Supino", 60, 10)], 50, "12")).toEqual({ carga: 60, reps: 10 });
    expect(valoresIniciais([], [], 50, "8-12")).toEqual({ carga: 50, reps: 8 });
    expect(valoresIniciais([], [], null, "até a falha")).toEqual({ carga: 0, reps: 10 });
  });

  it("última vez ignora hoje e acha o exercício sem acento nem maiúscula", () => {
    const sessoes = [
      { data: "2026-10-23", series: [serie("Supino reto", 70, 5)] },
      { data: "2026-10-20", series: [serie("supino  RETO", 60, 10), serie("Supino reto", 62.5, 8)] },
      { data: "2026-10-16", series: [serie("Supino reto", 55, 10)] },
    ];
    expect(ultimaVez(sessoes, "Supino Reto", "2026-10-23").map((s) => s.carga_kg)).toEqual([60, 62.5]);
    expect(ultimaVez(sessoes, "Agachamento", "2026-10-23")).toEqual([]);
  });

  it("lê a linha da ficha digitada", () => {
    expect(lerLinhaDeExercicio("Supino reto 4x8-10 60kg")).toEqual({ nome: "Supino reto", series: 4, repeticoes: "8-10", carga_kg: 60 });
    expect(lerLinhaDeExercicio("Crucifixo 3 x 12")).toEqual({ nome: "Crucifixo", series: 3, repeticoes: "12", carga_kg: null });
    expect(lerLinhaDeExercicio("Prancha")).toEqual({ nome: "Prancha", series: 3, repeticoes: "10", carga_kg: null });
    expect(lerLinhaDeExercicio("Leg 45 4x10 120,5 kg")?.carga_kg).toBe(120.5);
    expect(lerLinhaDeExercicio("   ")).toBeNull();
  });
});

it("datas", () => {
  expect(dataCurta("2026-10-23")).toBe("23/10");
  expect(somarDias("2026-10-26", -7)).toBe("2026-10-19");
  expect(somarDias("2026-12-28", 7)).toBe("2027-01-04");
});
