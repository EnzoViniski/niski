import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { usarTransporte } from "../mobile/api";
import { lerRota, urlDaRota } from "../mobile/rota";
import { criarTarefa } from "../features/tarefas/dominio";
import { reiniciarParaTeste } from "../features/tarefas/loja";

// Fork (Tarefas e Rotina no celular): as telas do PC pelo engine, com o transporte da rede.
let pedidos: string[] = [];
let gravadas: unknown[] = [];
const tarefa = { ...criarTarefa({ titulo: "Comprar café" }, "enzo"), id: "t-1" };
const dia = {
  data: "2026-10-27", dia_da_semana: "terça",
  itens: [{ id: "bloco:b1", tipo: "bloco", titulo: "Medza", detalhe: "", inicio: "09:00", fim: "12:00", area: "medza", cor: "#0ea5e9", estado: "", alvo: "b1", categoria: "trabalho" }],
  sem_hora: [], cancelados: [], agua: { ml: 500, meta_ml: 2500, copo_ml: 250 },
  alimentacao: { kcal: 0, proteina_g: 0, refeicoes: 0, agua_ml: 500 },
  treino: { ficha: null, sessao: null }, agora: "bloco:b1", proximo: null, foco: "Entregar a Aircook", metas: [],
};

beforeEach(() => {
  pedidos = []; gravadas = [];
  reiniciarParaTeste();
  usarTransporte(async (caminho, init) => {
    const metodo = init.method ?? "GET";
    pedidos.push(`${metodo} ${caminho}`);
    const json = (v: unknown, status = 200) => new Response(JSON.stringify(v), { status });
    if (caminho === "/tarefas/contexto") return json({ eu: { id: "enzo", nome: "Enzo", cor: "#f97316", atualizadaEm: "" }, compartilhada: false });
    if (caminho === "/tarefas/listar/tarefas") return json({ itens: [JSON.stringify(tarefa)] });
    if (caminho === "/tarefas/listar/pessoas") return json({ itens: [] });
    if (caminho === "/tarefas/gravar") { gravadas.push(JSON.parse(String(init.body))); return json({ ok: true }); }
    if (caminho.startsWith("/rotina/ler/ler_dia")) return json(dia);
    if (caminho === "/rotina/conversa" && metodo === "POST") return json({ conversa: "conv-rotina", sessao: "t1", nova: true });
    return json({ error: "não existe" }, 404);
  });
});
afterEach(() => usarTransporte((path, init) => fetch(path, init)));

describe("Tarefas e Rotina no celular", () => {
  it("rotas novas", () => {
    expect(lerRota("#/tarefas")).toEqual({ tela: "tarefas" });
    expect(lerRota("#/rotina")).toEqual({ tela: "rotina" });
    expect(urlDaRota({ tela: "rotina" })).toMatch(/#\/rotina$/);
  });

  it("Tarefas mostra as tarefas do PC com o eu do PC", async () => {
    const { Tarefas } = await import("../mobile/telas/Tarefas");
    render(<Tarefas voltar={() => {}} />);
    expect(await screen.findByText("Comprar café")).toBeInTheDocument();
    expect(pedidos).toContain("GET /tarefas/contexto");
    expect(pedidos).toContain("GET /tarefas/listar/tarefas");
    expect(JSON.parse(localStorage.getItem("medza-tarefas-ajustes") ?? "{}").eu.id).toBe("enzo");
  });

  it("Rotina mostra o dia e abre a conversa da rotina no chat", async () => {
    const { Rotina } = await import("../mobile/telas/Rotina");
    const abrir: string[] = [];
    render(<Rotina abrirConversaNoChat={(id) => abrir.push(id)} />);
    expect(await screen.findByText("Medza")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Conversar" }));
    await waitFor(() => expect(abrir).toEqual(["conv-rotina"]));
    expect(pedidos.some((p) => p.startsWith("GET /rotina/ler/ler_dia"))).toBe(true);
  });
});
