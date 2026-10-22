import { describe, expect, it } from "vitest";
import {
  concluir, criarTarefa, lerCriacaoRapida, mesclar, montarDia, normalizarTarefa, posicaoEntre, proximaData, rotuloDoDia, slug,
} from "./dominio";
import type { Pessoa } from "./tipos";

const HOJE = "2026-10-21"; // quarta-feira
const pessoas: Pessoa[] = [
  { id: "enzo", nome: "Enzo", cor: "#000", atualizadaEm: "" },
  { id: "joao-pedro", nome: "João Pedro", cor: "#111", atualizadaEm: "" },
];

describe("criação rápida", () => {
  it("entende data, hora, categoria, prioridade e pessoa", () => {
    const r = lerCriacaoRapida("Consulta dentista amanhã 14h #saude !alta @joão", HOJE, pessoas, "enzo");
    expect(r).toMatchObject({ titulo: "Consulta dentista", data: "2026-10-22", hora: "14:00", categoria: "saude", prioridade: "alta", responsavel: "joao-pedro" });
  });
  it("hora sozinha vale para hoje; dia da semana vai para a próxima", () => {
    expect(lerCriacaoRapida("Ligar 9:30", HOJE, pessoas, "enzo")).toMatchObject({ titulo: "Ligar", data: HOJE, hora: "09:30" });
    expect(lerCriacaoRapida("Prova na sexta", HOJE, pessoas, "enzo").data).toBe("2026-10-23");
    expect(lerCriacaoRapida("Reunião quarta", HOJE, pessoas, "enzo").data).toBe("2026-10-28");
  });
  it("dd/mm que já passou é do ano que vem; recorrência", () => {
    expect(lerCriacaoRapida("Renovar CNH 05/01", HOJE, pessoas, "enzo").data).toBe("2027-01-05");
    expect(lerCriacaoRapida("Remédio todo dia 8h", HOJE, pessoas, "enzo")).toMatchObject({ titulo: "Remédio", recorrencia: "diaria", hora: "08:00" });
  });
  it("marca desconhecida fica no título", () => {
    const r = lerCriacaoRapida("Comprar #xyz para @maria", HOJE, pessoas, "enzo");
    expect(r).toMatchObject({ titulo: "Comprar #xyz para @maria", categoria: "outro", responsavel: "enzo" });
  });
  it("#medico cai em saúde", () => {
    expect(lerCriacaoRapida("Exame #medico", HOJE, pessoas, "enzo").categoria).toBe("saude");
  });
});

describe("recorrência", () => {
  it("dias úteis pulam o fim de semana e mensal respeita fim de mês", () => {
    expect(proximaData("2026-10-23", "dias_uteis", HOJE)).toBe("2026-10-26");
    expect(proximaData("2026-01-31", "mensal", HOJE)).toBe("2026-02-28");
    expect(proximaData("", "diaria", HOJE)).toBe("2026-10-22");
  });
  it("concluir recorrente volta para A fazer na próxima data com checklist zerado", () => {
    const t = criarTarefa({ titulo: "Remédio", data: HOJE, recorrencia: "diaria", checklist: [{ id: "1", texto: "x", feito: true }] }, "enzo");
    const feita = concluir(t, new Date(2026, 9, 21, 10));
    expect(feita).toMatchObject({ status: "a_fazer", data: "2026-10-22" });
    expect(feita.checklist[0].feito).toBe(false);
    expect(concluir({ ...t, recorrencia: "nunca" }).status).toBe("feito");
  });
});

describe("mescla e leitura", () => {
  it("vale a mais nova e junta comentários das duas", () => {
    const base = criarTarefa({ titulo: "A" }, "enzo");
    const minha = { ...base, titulo: "Minha", atualizadaEm: "2026-10-21T10:00:00.000Z", comentarios: [{ id: "c1", autor: "enzo", texto: "oi", em: "2026-10-21T09:00:00.000Z" }] };
    const dele = { ...base, titulo: "Dele", atualizadaEm: "2026-10-21T11:00:00.000Z", comentarios: [{ id: "c2", autor: "joao", texto: "e aí", em: "2026-10-21T09:30:00.000Z" }] };
    const m = mesclar(minha, dele);
    expect(m.titulo).toBe("Dele");
    expect(m.comentarios.map((c) => c.id)).toEqual(["c1", "c2"]);
  });
  it("campo inválido cai no padrão sem perder a tarefa; id inválido é descartado", () => {
    const t = normalizarTarefa({ id: "t-1", titulo: "X", status: "???", hora: "25:00", checklist: "nada" });
    expect(t).toMatchObject({ status: "a_fazer", hora: "", checklist: [] });
    expect(normalizarTarefa({ id: "../x" })).toBeNull();
  });
});

describe("meu dia", () => {
  it("separa atrasadas, hoje (com hora primeiro), sem data e próximas", () => {
    const t = (campos: Parameters<typeof criarTarefa>[0]) => criarTarefa(campos, "enzo");
    const dia = montarDia([
      t({ titulo: "atrasada", data: "2026-10-20" }),
      t({ titulo: "tarde", data: HOJE, hora: "15:00" }),
      t({ titulo: "manhã", data: HOJE, hora: "08:00" }),
      t({ titulo: "sem hora", data: HOJE, prioridade: "urgente" }),
      t({ titulo: "solta" }),
      t({ titulo: "sexta", data: "2026-10-23" }),
      t({ titulo: "apagada", data: HOJE, apagada: true }),
    ], HOJE);
    expect(dia.atrasadas.map((x) => x.titulo)).toEqual(["atrasada"]);
    expect(dia.hoje.map((x) => x.titulo)).toEqual(["manhã", "tarde", "sem hora"]);
    expect(dia.semData.map((x) => x.titulo)).toEqual(["solta"]);
    expect(dia.proximas.map((x) => x.titulo)).toEqual(["sexta"]);
  });
});

describe("utilidades", () => {
  it("posição entre vizinhos, rótulo do dia e slug", () => {
    expect(posicaoEntre(10, 20)).toBe(15);
    expect(posicaoEntre(undefined, 20)).toBe(20 - 1024);
    expect(rotuloDoDia("2026-10-22", HOJE)).toBe("Amanhã");
    expect(slug("João Pedro")).toBe("joao-pedro");
  });
});
