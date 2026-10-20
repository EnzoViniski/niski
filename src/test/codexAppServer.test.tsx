import { afterEach, describe, expect, it } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { usarTransporte } from "../mobile/api";
import { CodexAppServer } from "../features/agentes/CodexAppServer";
import { CartaoDePermissao } from "../mobile/conversa/CartaoDePermissao";

afterEach(() => usarTransporte((path, init) => fetch(path, init)));
describe("Codex app-server por agente", () => {
  it("começa em Terminal e só altera a conversa escolhida", async () => {
    let ligados = [false, false]; const posts: unknown[] = [];
    usarTransporte(async (_path, init) => {
      if (init.method === "POST") { const body = JSON.parse(String(init.body)); posts.push(body); ligados = [body.ligado, false]; }
      return new Response(JSON.stringify({ versao_conferida: "0.163.0", suportado: true,
        agentes: ["A", "B"].map((nome, i) => ({ id: nome, nome, ligado: ligados[i], ativo: false })) }));
    });
    render(<CodexAppServer />);
    const a = await screen.findByRole("combobox", { name: "Conexão de A" });
    expect(a).toHaveValue("terminal");
    fireEvent.change(a, { target: { value: "app-server" } });
    await waitFor(() => expect(a).toHaveValue("app-server"));
    expect(screen.getByRole("combobox", { name: "Conexão de B" })).toHaveValue("terminal");
    expect(posts).toEqual([{ agente: "A", ligado: true }]);
  });
  it("falha ao salvar mantém a seleção anterior", async () => {
    usarTransporte(async (_path, init) => init.method === "POST" ? new Response("{}", { status: 500 }) :
      new Response(JSON.stringify({ versao_conferida: "0.163.0", suportado: true, agentes: [{ id: "a", nome: "A", ligado: false, ativo: false }] })));
    render(<CodexAppServer />);
    const select = await screen.findByRole("combobox");
    fireEvent.change(select, { target: { value: "app-server" } });
    expect(await screen.findByRole("alert")).toHaveTextContent("Não consegui salvar");
    expect(select).toHaveValue("terminal");
  });
  it("avisa quando o engine ainda não tem a opção", async () => {
    usarTransporte(async () => new Response("{}", { status: 404 }));
    render(<CodexAppServer />);
    expect(await screen.findByRole("alert")).toHaveTextContent("Engine antigo");
  });
  it("aprova o pedido do Codex pela mesma rota, sem ações de gancho", async () => {
    const enviados: unknown[] = [];
    usarTransporte(async (path, init) => {
      enviados.push([path, JSON.parse(String(init.body))]);
      return new Response(JSON.stringify({ state: "sent" }));
    });
    render(<CartaoDePermissao busy={false} onResponder={() => { throw new Error("Não deve digitar na PTY"); }} conversaId="c"
      pedido={{ id: "codex-pedido", origem: "codex_app_server", ferramenta: "Bash", resumo: "echo sintético", expira_em_ms: Date.now() + 600000 }} />);
    expect(screen.queryByRole("button", { name: "Responder no terminal" })).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Permitir" }));
    await waitFor(() => expect(enviados).toEqual([["/conversas/c/aprovar", { permitir: true, pedido: "codex-pedido" }]]));
  });
});
