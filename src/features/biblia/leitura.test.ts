import { beforeEach, describe, expect, it } from "vitest";
import { lerBiblia } from "./biblia";
import { arquivoDeTeste } from "./bibliaDeTeste";
import { decidir, deveAbrirSozinha, diasSeguidos, estadoDaLeitura, leituraDeAgora, lidasHoje, marcarComoLida, pendenteHoje, recarregarDaBiblia, salvarAjustesDaBiblia } from "./leitura";

const biblia = lerBiblia(arquivoDeTeste({ capitulos: 20 }));
const dia = (d: number, h = 7) => new Date(2026, 9, d, h, 0);
const manhaFeita = { ativo: true, horario: "06:00", terminouHoje: true };

beforeEach(() => { localStorage.clear(); recarregarDaBiblia(); });

describe("Bíblia do dia: o que ler e quando", () => {
  it("uma passagem por trilha escolhida, na ordem das trilhas", () => {
    salvarAjustesDaBiblia({ trilhas: ["sabedoria", "evangelhos"] });
    expect(leituraDeAgora(biblia).map((l) => l.trilha)).toEqual(["evangelhos", "sabedoria"]);
  });

  it("'Li' avança; 'amanhã' e 'mais tarde' guardam a mesma leitura", () => {
    const hoje = leituraDeAgora(biblia).map((l) => l.passagem.id);
    decidir("amanha", dia(1));
    expect(leituraDeAgora(biblia).map((l) => l.passagem.id)).toEqual(hoje);
    decidir("depois", dia(2));
    expect(leituraDeAgora(biblia).map((l) => l.passagem.id)).toEqual(hoje);
    marcarComoLida(biblia, leituraDeAgora(biblia), ["x"], dia(2));
    const depois = leituraDeAgora(biblia).map((l) => l.passagem.id);
    expect(depois.some((id) => hoje.includes(id))).toBe(false);
    expect(lidasHoje(biblia, estadoDaLeitura(), dia(2)).map((p) => p.id)).toEqual(hoje);
  });

  it("não repete em 15 dias seguidos de leitura", () => {
    const vistas: string[] = [];
    for (let d = 1; d <= 15; d++) {
      const leitura = leituraDeAgora(biblia);
      vistas.push(...leitura.map((l) => l.passagem.id));
      marcarComoLida(biblia, leitura, [], dia(d));
    }
    expect(new Set(vistas).size).toBe(vistas.length);
    expect(diasSeguidos(estadoDaLeitura(), dia(15))).toBe(15);
  });

  it("abre sozinha só depois dos exercícios e uma vez por dia", () => {
    const g = () => estadoDaLeitura();
    expect(deveAbrirSozinha(g(), { ...manhaFeita, terminouHoje: false }, dia(3))).toBe(false);
    expect(deveAbrirSozinha(g(), manhaFeita, dia(3))).toBe(true);
    decidir("depois", dia(3));
    expect(deveAbrirSozinha(g(), manhaFeita, dia(3))).toBe(false);
    expect(pendenteHoje(g(), dia(3))).toBe(true);
    expect(deveAbrirSozinha(g(), manhaFeita, dia(4))).toBe(true);
    // Exercícios desligados: vale o horário da manhã.
    expect(deveAbrirSozinha(g(), { ativo: false, horario: "06:00", terminouHoje: false }, dia(4, 5))).toBe(false);
    expect(deveAbrirSozinha(g(), { ativo: false, horario: "06:00", terminouHoje: false }, dia(4, 6))).toBe(true);
    salvarAjustesDaBiblia({ ativa: false });
    expect(deveAbrirSozinha(g(), manhaFeita, dia(4))).toBe(false);
  });

  it("ajustes estranhos no armazenamento viram o padrão", () => {
    localStorage.setItem("medza-biblia", JSON.stringify({ ajustes: { trilhas: ["inventada", "salmos"], letra: 3 }, marcadores: { salmos: { livro: "x" } }, hoje: { dia: 1 } }));
    recarregarDaBiblia();
    expect(estadoDaLeitura().ajustes.trilhas).toEqual(["salmos"]);
    expect(estadoDaLeitura().marcadores).toEqual({});
    expect(estadoDaLeitura().hoje).toBeNull();
  });
});
