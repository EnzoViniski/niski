import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { PermissoesDoAntigravity } from "../features/antigravity/PermissoesDoAntigravity";
import { transporteDoEngine } from "../features/conversa/transporteDoEngine";
import { usarTransporte } from "../mobile/api";

// Fork: Configurações → Agentes: o que o Antigravity aprova sozinho nas sessões do app.
const engine = vi.hoisted(() => ({ engineApi: vi.fn() }));
vi.mock("../features/terminal/terminalService", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../features/terminal/terminalService")>()),
  ...engine,
}));

let estado = { modo: "accept-edits", efetivo: "accept-edits", auto_disponivel: true };
beforeEach(() => {
  vi.clearAllMocks();
  estado = { modo: "accept-edits", efetivo: "accept-edits", auto_disponivel: true };
  usarTransporte(transporteDoEngine);
  engine.engineApi.mockImplementation(async (metodo: string, _caminho: string, corpo: string | null) => {
    if (metodo === "POST") { const modo = JSON.parse(String(corpo)).modo; estado = { ...estado, modo, efetivo: modo }; }
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
});
