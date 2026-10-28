import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { PermissoesDoAntigravity } from "../features/antigravity/PermissoesDoAntigravity";
import { transporteDoEngine } from "../features/conversa/transporteDoEngine";
import { usarTransporte } from "../mobile/api";

// Configurações → Agentes: o que o Antigravity aprova sozinho nas sessões do app.
const engine = vi.hoisted(() => ({ engineApi: vi.fn() }));
vi.mock("../features/terminal/terminalService", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../features/terminal/terminalService")>()),
  ...engine,
}));

let estado: Record<string, unknown> = { modo: "accept-edits", efetivo: "accept-edits", auto_disponivel: true };
beforeEach(() => {
  vi.clearAllMocks();
  estado = { modo: "accept-edits", efetivo: "accept-edits", auto_disponivel: true, comandos_liberados: 0, comandos_comuns: ["ls", "git status"] };
  usarTransporte(transporteDoEngine);
  engine.engineApi.mockImplementation(async (metodo: string, _caminho: string, corpo: string | null) => {
    if (metodo === "POST") {
      const pedido = JSON.parse(String(corpo));
      estado = "liberar_comuns" in pedido ? { ...estado, comandos_liberados: pedido.liberar_comuns ? 59 : 0 } : { ...estado, modo: pedido.modo, efetivo: pedido.modo };
    }
    return { type: "api", status: 200, body: JSON.stringify(estado) };
  });
});
afterEach(() => usarTransporte((path, init) => fetch(path, init)));

describe("permissões do Antigravity", () => {
  it("padrão: arquivos sem perguntar; troca pela rota do engine", async () => {
    render(<PermissoesDoAntigravity />);
    await waitFor(() => expect(screen.getByRole("radio", { name: /Arquivos sem perguntar/ })).toBeChecked());
    await userEvent.click(screen.getByRole("radio", { name: /Perguntar tudo/ }));
    await waitFor(() => expect(screen.getByRole("radio", { name: /Perguntar tudo/ })).toBeChecked());
    expect(engine.engineApi).toHaveBeenCalledWith("POST", "/antigravity/permissoes", JSON.stringify({ modo: "padrao" }), expect.anything());
  });

  it("Automático escolhido mas a conta não tem: avisa que vale arquivos sem perguntar", async () => {
    estado = { modo: "auto", efetivo: "accept-edits", auto_disponivel: false };
    render(<PermissoesDoAntigravity />);
    expect(await screen.findByText(/ainda não liberou o Automático/)).toBeInTheDocument();
    expect(screen.getByRole("radio", { name: /Automático/ })).toBeChecked();
  });

  // Fork (23/10/2026): pedido do Enzo, o Antigravity sem um trilhão de perguntas.
  it("Sem perguntar é a primeira opção e avisa quando o agy é antigo", async () => {
    estado = { modo: "sem-perguntar", efetivo: "accept-edits", auto_disponivel: false, sem_perguntar_disponivel: false };
    render(<PermissoesDoAntigravity />);
    await waitFor(() => expect(screen.getByRole("radio", { name: /^Sem perguntar/ })).toBeChecked());
    expect(screen.getAllByRole("radio")[0]).toHaveAccessibleName(/^Sem perguntar \(recomendado\)/);
    expect(screen.getByText(/atualize o agy/)).toBeInTheDocument();
  });

  it("libera e desfaz os comandos de pouco risco no Antigravity", async () => {
    render(<PermissoesDoAntigravity />);
    await userEvent.click(await screen.findByRole("button", { name: "Liberar comandos comuns" }));
    expect(await screen.findByText("59 comando(s) liberado(s) pelo app")).toBeInTheDocument();
    expect(engine.engineApi).toHaveBeenCalledWith("POST", "/antigravity/permissoes", JSON.stringify({ liberar_comuns: true }), expect.anything());
    await userEvent.click(screen.getByRole("button", { name: "Desfazer" }));
    expect(await screen.findByRole("button", { name: "Liberar comandos comuns" })).toBeInTheDocument();
  });
});
