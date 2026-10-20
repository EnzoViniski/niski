import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { AgentLauncher } from "../features/terminal/AgentLauncher";
import { escolherConta } from "../features/roteamento/roteamentoService";
import { contaDoAgente } from "../lib/escolhasSalvas";

// Fork (3B): conta "Automática (fila)" no Novo agente — a fila escolhe e a tela diz por quê.
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("../features/roteamento/roteamentoService", async (original) => ({
  ...(await original<typeof import("../features/roteamento/roteamentoService")>()),
  escolherConta: vi.fn(),
}));

const perfil = (id: string, name: string) => ({ id, provider: "claude", name, config_dir: `/c/${id}`, builtin: false,
  created_at_ms: 0, last_used_at_ms: null, authenticated: true });

beforeEach(() => {
  localStorage.clear();
  vi.mocked(invoke).mockReset().mockImplementation(async (command: string) => {
    switch (command) {
      case "agent_cli_statuses":
        return [{ id: "claude", label: "Claude", command: "claude", path: "x", available: true, authenticated: true }];
      case "list_profiles": return [perfil("a", "Pessoal"), perfil("b", "Empresa")];
      case "git_branches": return [];
      default: return null;
    }
  });
});

async function escolherAutomatica() {
  fireEvent.click(await screen.findByRole("combobox", { name: /Conta/ }));
  fireEvent.click(await screen.findByRole("option", { name: /Automática \(fila\)/ }));
}

describe("AgentLauncher — conta automática", () => {
  it("abre na conta que a fila escolheu, mostrando o porquê, e lembra a escolha", async () => {
    vi.mocked(escolherConta).mockResolvedValue({ perfil_id: "b", nome: "Empresa",
      explicacao: ["Pessoal (a): pulada — limite estourado (5h)", "Empresa (b): escolhida — primeira da fila que passou nos filtros (5h 10%)"] });
    const onLaunch = vi.fn();
    render(<AgentLauncher projectName="p" projectPath="/p" onLaunch={onLaunch} />);
    await escolherAutomatica();
    expect(await screen.findByText("Vai abrir em Empresa.")).toBeInTheDocument();
    expect(screen.getByText(/Pessoal \(a\): pulada — limite estourado/)).toBeInTheDocument();
    expect(escolherConta).toHaveBeenCalledWith("claude", "/p");
    const abrir = screen.getByRole("button", { name: /Abrir Claude/ });
    await waitFor(() => expect(abrir).not.toBeDisabled());
    fireEvent.click(abrir);
    await waitFor(() => expect(onLaunch).toHaveBeenCalledWith(expect.objectContaining({ profile: expect.objectContaining({ id: "b" }) })));
    expect(contaDoAgente("claude")).toBe("auto");
  });

  it("sem conta apta na fila, não abre e explica", async () => {
    vi.mocked(escolherConta).mockResolvedValue({ perfil_id: null, nome: null, explicacao: ["nenhuma conta da fila passou nos filtros"] });
    render(<AgentLauncher projectName="p" projectPath="/p" onLaunch={vi.fn()} />);
    await escolherAutomatica();
    expect(await screen.findByText(/Nenhuma conta da fila serve agora/)).toBeInTheDocument();
    expect(screen.getByText("nenhuma conta da fila passou nos filtros")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Abrir Claude/ })).toBeDisabled();
  });

  it("engine sem a rota: avisa e não inventa conta", async () => {
    vi.mocked(escolherConta).mockRejectedValue(new Error("Not Found"));
    render(<AgentLauncher projectName="p" projectPath="/p" onLaunch={vi.fn()} />);
    await escolherAutomatica();
    expect(await screen.findByText(/Não deu para consultar a fila: Not Found/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Abrir Claude/ })).toBeDisabled();
  });

  it("uma conta só: sem a opção Automática", async () => {
    vi.mocked(invoke).mockImplementation(async (command: string) =>
      command === "agent_cli_statuses" ? [{ id: "claude", label: "Claude", command: "claude", path: "x", available: true, authenticated: true }]
        : command === "list_profiles" ? [perfil("a", "Pessoal")] : command === "git_branches" ? [] : null);
    render(<AgentLauncher projectName="p" projectPath="/p" onLaunch={vi.fn()} />);
    await screen.findByRole("button", { name: /Abrir Claude/ });
    expect(screen.queryByRole("combobox", { name: /Conta/ })).not.toBeInTheDocument();
  });
});
