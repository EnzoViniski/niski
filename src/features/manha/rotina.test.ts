import { beforeEach, describe, expect, it } from "vitest";
import {
  AJUSTES_PADRAO, adiar, deveMostrar, estadoDaManha, iniciarSessao, montarQuestoes, nivelDoDia, pularHoje, recarregarDaManha,
  responder, salvarAjustesDaManha, sequencia, terminar, type AjustesDaManha,
} from "./rotina";
import { questoesDeCards, lerQuestoesDaIa } from "./fontes";
import { criarRng, type Questao } from "./gerador";
import type { CardDeRevisao } from "../roadmaps/tipos";

const as = (h: number, m = 0) => new Date(2026, 9, 21, h, m);

beforeEach(() => { localStorage.clear(); recarregarDaManha(); });

describe("quando aparece", () => {
  it("só depois do horário, uma vez por dia, respeitando adiar e pular", () => {
    expect(deveMostrar(estadoDaManha(), as(5, 59))).toBe(false);
    expect(deveMostrar(estadoDaManha(), as(6, 0))).toBe(true);
    // PC ligado só à tarde: aparece ao abrir.
    expect(deveMostrar(estadoDaManha(), as(15, 0))).toBe(true);

    adiar(30, as(7, 0));
    expect(deveMostrar(estadoDaManha(), as(7, 10))).toBe(false);
    expect(deveMostrar(estadoDaManha(), as(7, 31))).toBe(true);

    pularHoje(as(7, 40));
    expect(deveMostrar(estadoDaManha(), as(20, 0))).toBe(false);
    // No dia seguinte volta.
    expect(deveMostrar(estadoDaManha(), new Date(2026, 9, 22, 6, 1))).toBe(true);
  });

  it("desligado ou sem temas não aparece; horário configurável", () => {
    salvarAjustesDaManha({ horario: "08:30" });
    expect(deveMostrar(estadoDaManha(), as(8, 0))).toBe(false);
    expect(deveMostrar(estadoDaManha(), as(8, 30))).toBe(true);
    salvarAjustesDaManha({ ativo: false });
    expect(deveMostrar(estadoDaManha(), as(9))).toBe(false);
    salvarAjustesDaManha({ ativo: true, temas: [] });
    expect(deveMostrar(estadoDaManha(), as(9))).toBe(false);
  });

  it("sessão feita some até amanhã e entra no histórico", () => {
    const q: Questao = { id: "1", tema: "aritmetica", enunciado: "1+1", opcoes: ["2", "3", "4", "5"], correta: 0, explicacao: "" };
    iniciarSessao([q, { ...q, id: "2" }], as(6, 5));
    responder(0, 0, 3);
    responder(1, 2, 4);
    responder(1, 0, 4); // segunda resposta não vale
    terminar(as(6, 7));
    const g = estadoDaManha();
    expect(g.historico[g.historico.length - 1]).toMatchObject({ dia: "2026-10-21", acertos: 1, total: 2, segundos: 7 });
    expect(deveMostrar(g, as(10))).toBe(false);
  });
});

describe("sequência e nível", () => {
  it("conta dias seguidos até hoje ou ontem", () => {
    const h = (dia: string) => ({ dia, acertos: 1, total: 1, segundos: 1 });
    expect(sequencia([h("2026-10-19"), h("2026-10-20"), h("2026-10-21")], "2026-10-21")).toBe(3);
    expect(sequencia([h("2026-10-19"), h("2026-10-20")], "2026-10-21")).toBe(2);
    expect(sequencia([h("2026-10-18")], "2026-10-21")).toBe(0);
  });
  it("adaptativa sobe com acerto alto e desce com baixo", () => {
    const a: AjustesDaManha = { ...AJUSTES_PADRAO, dificuldade: "adaptativa" };
    const dias = (acertos: number) => [1, 2, 3].map((i) => ({ dia: `2026-10-1${i}`, acertos, total: 5, segundos: 60 }));
    expect(nivelDoDia(a, dias(5))).toBe(3);
    expect(nivelDoDia(a, dias(1))).toBe(1);
    expect(nivelDoDia(a, dias(3))).toBe(2);
    expect(nivelDoDia({ ...a, dificuldade: "facil" }, dias(5))).toBe(1);
  });
});

describe("montar as questões do dia", () => {
  it("respeita a quantidade e troca fonte vazia por matemática", async () => {
    const ajustes = { ...AJUSTES_PADRAO, quantidade: 6, temas: ["roadmaps", "livre", "logica"] as AjustesDaManha["temas"], temaLivre: "inglês" };
    const questoes = await montarQuestoes(ajustes, [], "2026-10-21", {
      roadmaps: async () => [],
      livre: async () => { throw new Error("IA fora do ar"); },
    });
    expect(questoes).toHaveLength(6);
    expect(questoes.every((q) => q.tema === "logica")).toBe(true);
    expect(new Set(questoes.map((q) => q.id)).size).toBe(6);
  });

  it("usa o que as fontes extras entregam", async () => {
    const ia: Questao = { id: "x", tema: "livre", enunciado: "Hello =", opcoes: ["Olá", "Tchau", "Sim", "Não"], correta: 0, explicacao: "" };
    const ajustes = { ...AJUSTES_PADRAO, quantidade: 2, temas: ["livre"] as AjustesDaManha["temas"], temaLivre: "inglês" };
    const questoes = await montarQuestoes(ajustes, [], "2026-10-21", { livre: async () => [ia, { ...ia, enunciado: "Bye =" }] });
    expect(questoes.map((q) => q.enunciado)).toEqual(["Hello =", "Bye ="]);
  });
});

describe("fontes extras", () => {
  const card = (id: string, frente: string, verso: string, due = "2026-01-01T00:00:00.000Z"): { card: CardDeRevisao; roadmap: string; nome: string } => ({
    roadmap: "r1", nome: "Inglês",
    card: { id, etapaId: "e", frente, verso, criadoEm: due, fsrs: { due, stability: 0, difficulty: 0, elapsed_days: 0, scheduled_days: 0, learning_steps: 0, reps: 0, lapses: 0, state: 0, last_review: null } },
  });
  it("cards viram questões com as outras respostas de alternativa; menos de 4 cards não dá", () => {
    const cards = [card("a", "dog", "cachorro"), card("b", "cat", "gato"), card("c", "bird", "pássaro"), card("d", "fish", "peixe", "2099-01-01T00:00:00.000Z")];
    const qs = questoesDeCards(cards, 2, criarRng(1), new Date(2026, 9, 21));
    expect(qs).toHaveLength(2);
    expect(qs.every((q) => q.opcoes.length === 4 && q.origem?.roadmap === "r1")).toBe(true);
    expect(qs.map((q) => q.enunciado)).not.toContain("fish"); // vencidos primeiro
    expect(questoesDeCards(cards.slice(0, 3), 2, criarRng(1))).toEqual([]);
  });
  it("resposta da IA: descarta questão mal formada", () => {
    const lidas = lerQuestoesDaIa({ questoes: [
      { enunciado: "ok", opcoes: ["a", "b", "c", "d"], correta: 2, explicacao: "x" },
      { enunciado: "repetidas", opcoes: ["a", "a", "c", "d"], correta: 0 },
      { enunciado: "índice", opcoes: ["a", "b", "c", "d"], correta: 7 },
    ] }, 5);
    expect(Array.isArray(lidas) && lidas.map((q) => q.enunciado)).toEqual(["ok"]);
    expect(typeof lerQuestoesDaIa({}, 5)).toBe("string");
  });
});
