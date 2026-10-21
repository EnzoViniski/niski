import { fireEvent, render, screen, within } from "@testing-library/react";
import { afterAll, beforeEach, describe, expect, it } from "vitest";
import { usarTransporte } from "../mobile/api";
import { Roadmaps } from "../mobile/telas/Roadmaps";
import { gravacoesPendentes } from "../features/roadmaps/armazenamento";
import { limparLoja } from "../features/roadmaps/loja";
import { lerRota, urlDaRota } from "../mobile/rota";

/** Engine falso: as rotas `/roadmaps` sobre um mapa de arquivos (o que o PC gravou). */
function engineFalso() {
  const arquivos = new Map<string, string>();
  const json = (corpo: unknown, status = 200) => new Response(JSON.stringify(corpo), { status, headers: { "Content-Type": "application/json" } });
  const meta = { versao: 1, id: "r-vendas", nome: "Vendas", cor: "#22a06b", icone: "dinheiro", arquivado: false, fase: "ativo", criadoEm: "2026-10-21T10:00:00Z", atualizadoEm: "2026-10-21T10:00:00Z", ia: { cli: "claude" }, resumo: { etapas: 2, concluidas: 0, sequencia: 0, cardsParaHoje: 0, proximaEtapa: "Rapport" } };
  arquivos.set("r-vendas/meta.json", JSON.stringify(meta));
  arquivos.set("r-vendas/plano.json", JSON.stringify({ versao: 1, objetivo: "Vender", visao: "", modulos: [{ id: "m-1", titulo: "Base", descricao: "", etapas: [
    { id: "e-rapport", titulo: "Rapport", objetivo: "Criar sintonia", minutos: 20, tarefa: "Ligue para 2 clientes", fontes: [{ titulo: "Guia", url: "https://exemplo.com/guia", tipo: "artigo", gratis: true }], tarefas: [], preparada: true },
    { id: "e-spin", titulo: "SPIN", objetivo: "", minutos: 25, tarefa: "", fontes: [], tarefas: [] },
  ] }] }));
  arquivos.set("r-vendas/etapas/e-rapport.md", "## Rapport\n\nSintonia antes de vender.");
  const pedidos: string[] = [];
  const responder = async (caminho: string, init: RequestInit) => {
    pedidos.push(`${init.method ?? "GET"} ${caminho}`);
    const url = new URL(caminho, "http://celular");
    if (url.pathname === "/roadmaps") return json([{ id: "r-vendas", meta: JSON.parse(arquivos.get("r-vendas/meta.json") as string) }]);
    const [, id, acao] = url.pathname.match(/^\/roadmaps\/([^/]+)\/(arquivo|conversa)$/) ?? [];
    if (acao === "arquivo" && (init.method ?? "GET") === "GET") return json({ conteudo: arquivos.get(`${id}/${url.searchParams.get("nome")}`) ?? null });
    if (acao === "arquivo") { const { nome, conteudo } = JSON.parse(init.body as string) as { nome: string; conteudo: string }; arquivos.set(`${id}/${nome}`, conteudo); return json({ ok: true }); }
    return json({ error: "não existe" }, 404);
  };
  return { arquivos, pedidos, responder };
}

let engine: ReturnType<typeof engineFalso>;
beforeEach(() => {
  limparLoja();
  engine = engineFalso();
  usarTransporte((caminho, init) => engine.responder(caminho, init));
});
afterAll(() => usarTransporte((caminho, init) => fetch(caminho, init)));

describe("Roadmaps no celular", () => {
  it("rota #/roadmaps e #/roadmaps/<id>", () => {
    expect(lerRota("#/roadmaps")).toEqual({ tela: "roadmaps" });
    expect(lerRota("#/roadmaps/r-vendas")).toEqual({ tela: "roadmaps", roadmap: "r-vendas" });
    expect(lerRota("#/roadmaps/../x")).toEqual({ tela: "inicio" });
    expect(urlDaRota({ tela: "roadmaps", roadmap: "r-vendas" })).toMatch(/#\/roadmaps\/r-vendas$/);
  });

  it("lista, abre a etapa, conclui, escreve card e caderno: tudo vai para os arquivos do PC", async () => {
    let aberto: string | null = null;
    const { rerender } = render(<Roadmaps roadmap={null} abrir={(id) => { aberto = id; }} />);
    fireEvent.click(await screen.findByRole("button", { name: "Abrir Vendas" }));
    expect(aberto).toBe("r-vendas");
    rerender(<Roadmaps roadmap="r-vendas" abrir={() => undefined} />);
    fireEvent.click(await screen.findByRole("button", { name: "Rapport: Disponível" }));
    expect(await screen.findByText("Sintonia antes de vender.")).toBeInTheDocument();
    // IA fica no PC: sem botões de IA.
    expect(screen.queryByRole("button", { name: /Gerar cards de revisão/ })).toBeNull();
    expect(screen.getByRole("link", { name: "Guia" })).toHaveAttribute("href", "https://exemplo.com/guia");

    fireEvent.click(screen.getByRole("button", { name: "Escrever um card" }));
    fireEvent.change(screen.getByLabelText("Frente do card"), { target: { value: "O que é rapport?" } });
    fireEvent.change(screen.getByLabelText("Verso do card"), { target: { value: "Sintonia" } });
    fireEvent.click(screen.getByRole("button", { name: "Guardar card" }));
    await screen.findByText("Card entrou na revisão de hoje.");
    fireEvent.click(screen.getByRole("button", { name: "Concluir etapa" }));
    await screen.findByText("Etapa concluída");
    await gravacoesPendentes();
    expect(JSON.parse(engine.arquivos.get("r-vendas/progresso.json") as string).etapas["e-rapport"].estado).toBe("concluida");
    expect(JSON.parse(engine.arquivos.get("r-vendas/cards.json") as string).cards[0].frente).toBe("O que é rapport?");
    expect(JSON.parse(engine.arquivos.get("r-vendas/meta.json") as string).resumo).toMatchObject({ concluidas: 1, cardsParaHoje: 1 });

    fireEvent.click(screen.getByRole("button", { name: "← Mapa" }));
    fireEvent.click(await screen.findByRole("button", { name: "Revisar (1)" }));
    const revisao = screen.getByRole("dialog", { name: "Revisão" });
    fireEvent.click(within(revisao).getByRole("button", { name: "Mostrar resposta" }));
    fireEvent.click(within(revisao).getByRole("button", { name: /Bom/ }));
    await within(revisao).findByText("1 feitas");
    fireEvent.click(within(revisao).getByRole("button", { name: "Fechar revisão" }));

    fireEvent.click(screen.getByRole("button", { name: "Caderno" }));
    const caderno = await screen.findByLabelText("Texto do caderno");
    fireEvent.change(caderno, { target: { value: "Aprendi rapport pelo celular." } });
    fireEvent.blur(caderno);
    await gravacoesPendentes();
    expect(engine.arquivos.get("r-vendas/caderno.md")).toContain("pelo celular");
    expect(screen.queryByRole("button", { name: "Pedir revisão à IA" })).toBeNull();
    expect(engine.pedidos.some((p) => p.startsWith("POST /roadmaps/r-vendas/arquivo"))).toBe(true);
  });
});
