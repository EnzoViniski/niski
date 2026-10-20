import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { retomarDoHistorico, type DependenciasDaRetomada } from "../features/retomada/retomarDoHistorico";
import {
  cliNaoAchouAConversa,
  ehPastaSumida,
  prepararRetomada,
  RetomadaError,
  semCodigo,
} from "../features/retomada/retomadaService";
import { AvisoDeAtualizacao, textosDaAtualizacao } from "../components/AvisoDeAtualizacao";
import { statusDaConversa } from "../components/ConversaStatus";
import { HistoryView } from "../features/history/HistoryView";
import type { HistoryEntry } from "../features/history/historyService";
import type { WorkspaceState } from "../types/workspace";

/*
 * Fork (retomar-sempre): retomar sempre tem saída. CLI e engine falsos: `invoke` responde por rota.
 */

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const entry: HistoryEntry = {
  provider: "claude", profile_id: "p1", profile_name: "Pessoal", session_id: "sess-1", title: "Login",
  first_prompt: "corrige o login", cwd: "/repo/app", started_at_ms: 1, updated_at_ms: 2, path: "/c/projects/x/sess-1.jsonl",
};

function workspace(tabs: string[] = []): WorkspaceState {
  return {
    version: 1, id: "w1", name: "W", activeProjectId: "proj", updatedAt: "",
    projects: [{
      id: "proj", name: "app", path: "/repo/app", gitRoot: null, branch: null, composeFile: null, lastOpenedAt: "",
      activePaneId: "pane", maximizedPaneId: null,
      layout: { type: "pane", id: "pane", activeTabId: null, tabs: tabs.map((id) => ({ id: `tab-${id}`, kind: "agent", title: id, resourceId: id })) },
    }],
  } as WorkspaceState;
}

let rotas: Record<string, { status: number; body: unknown }> = {};
const chamadas: { path: string; body: unknown }[] = [];

beforeEach(() => {
  rotas = {};
  chamadas.length = 0;
  vi.mocked(invoke).mockReset();
  vi.mocked(invoke).mockImplementation(async (comando: string, args?: unknown) => {
    const a = (args ?? {}) as Record<string, unknown>;
    switch (comando) {
      case "agent_cli_statuses":
        return [{ id: "claude", label: "Claude", command: "claude", path: "/bin/claude", available: true, authenticated: true }];
      case "ensure_engine":
        return { type: "pong", protocol_version: 1, engine_pid: 1 };
      case "ensure_agent_trust":
        return null;
      case "spawn_terminal":
        return { type: "session", session: { id: "nova-sessao", name: "Claude · Login", project_id: a.projectId } };
      case "engine_api": {
        const path = String(a.path);
        chamadas.push({ path, body: a.body ? JSON.parse(String(a.body)) : null });
        const rota = rotas[path] ?? { status: 404, body: "" };
        return { type: "api", status: rota.status, body: typeof rota.body === "string" ? rota.body : JSON.stringify(rota.body) };
      }
      default:
        return null;
    }
  });
});

function deps(extra: Partial<DependenciasDaRetomada> = {}) {
  return {
    workspaces: [workspace()], activeWorkspaceId: "w1", projetoAtual: { id: "proj", path: "/repo/app" },
    dispatch: vi.fn(), lembrarProjeto: vi.fn(), aoAbrir: vi.fn(), ...extra,
  } satisfies DependenciasDaRetomada;
}

const ok = { viva: null, pasta_existe: true, pasta_principal: null, transcript: true, conta: "p1", pronto: true };

describe("retomar do Histórico", () => {
  it("mantém o servidor local do Codex ao retomar pelo Histórico", async () => {
    vi.mocked(invoke).mockResolvedValueOnce([{ id: "codex", label: "Codex", command: "codex", available: true, authenticated: false, path: "/fake/codex" }]);
    rotas["/retomada/preparar"] = { status: 200, body: { ...ok, modelo_local: { provedor: "ollama", modelo: "empresa/modelo:local" } } };
    await retomarDoHistorico({ ...entry, provider: "codex" }, "p1", {}, deps());
    expect(invoke).toHaveBeenCalledWith("spawn_terminal", expect.objectContaining({
      initialCommand: "codex resume sess-1 --oss --local-provider ollama --model empresa/modelo:local",
    }));
  });
  it("não abre conversa quando a preparação falha e não disfarça erro como engine antigo", async () => {
    rotas["/retomada/preparar"] = { status: 500, body: { error: "Falha de disco" } };
    await expect(retomarDoHistorico(entry, "p1", {}, deps())).rejects.toThrow("Falha de disco");
    expect(vi.mocked(invoke).mock.calls.some(([cmd]) => cmd === "spawn_terminal")).toBe(false);
  });

  it("não abre uma conversa sem o histórico que foi encontrado mas não pôde ser copiado", async () => {
    rotas["/retomada/preparar"] = { status: 200, body: { ...ok, pronto: false } };
    await expect(retomarDoHistorico(entry, "p1", {}, deps())).rejects.toThrow("não pôde ser preparado");
    expect(vi.mocked(invoke).mock.calls.some(([cmd]) => cmd === "spawn_terminal")).toBe(false);
  });
  it("conversa já viva numa aba: foca a aba e não abre um segundo --resume", async () => {
    rotas["/retomada/preparar"] = { status: 200, body: { ...ok, viva: { terminal_session_id: "viva-1", project_id: "proj" } } };
    const d = deps({ workspaces: [workspace(["viva-1"])] });
    await retomarDoHistorico(entry, "p1", {}, d);
    expect(d.dispatch).toHaveBeenCalledWith(expect.objectContaining({ type: "FOCUS_SESSION", sessionId: "viva-1", projectId: "proj" }));
    expect(invoke).not.toHaveBeenCalledWith("spawn_terminal", expect.anything());
  });

  it("transcript em outra conta: o engine prepara, e a CLI abre com --resume na conta escolhida", async () => {
    rotas["/retomada/preparar"] = { status: 200, body: { ...ok, conta: "p2" } };
    const d = deps();
    await retomarDoHistorico(entry, "p2", {}, d);
    expect(chamadas[0]).toEqual({ path: "/retomada/preparar", body: { provider: "claude", session_id: "sess-1", profile_id: "p2", cwd: "/repo/app", preparar: true } });
    expect(invoke).toHaveBeenCalledWith("spawn_terminal", expect.objectContaining({ initialCommand: "claude --resume sess-1", profileId: "p2" }));
    expect(chamadas.some((c) => c.path === "/historico/levar")).toBe(false);
    expect(d.dispatch).toHaveBeenCalledWith(expect.objectContaining({ type: "FOCUS_SESSION", sessionId: "nova-sessao" }));
  });

  it("engine antigo (sem a rota nova): segue pelo caminho de antes, sem quebrar", async () => {
    rotas["/historico/levar"] = { status: 200, body: { ok: true } };
    await retomarDoHistorico(entry, "p2", {}, deps());
    expect(chamadas.map((c) => c.path)).toEqual(["/retomada/preparar", "/historico/levar"]);
    expect(invoke).toHaveBeenCalledWith("spawn_terminal", expect.objectContaining({ initialCommand: "claude --resume sess-1" }));
  });

  it("pasta apagada: erro com 'Abrir no repositório principal' e 'Conversa nova lá'", async () => {
    rotas["/retomada/preparar"] = { status: 200, body: { ...ok, pasta_existe: false, pasta_principal: "/repo/principal" } };
    const erro = await retomarDoHistorico(entry, "p1", {}, deps()).catch((e: unknown) => e);
    expect(erro).toBeInstanceOf(RetomadaError);
    const acoes = (erro as RetomadaError).acoes;
    expect(acoes.map((a) => a.rotulo)).toEqual(["Abrir no repositório principal", "Conversa nova lá, com o histórico"]);
    expect(acoes[0].opcoes).toEqual({ cwd: "/repo/principal" });
    expect(invoke).not.toHaveBeenCalledWith("spawn_terminal", expect.anything());
  });

  it("pasta não registrada: oferece o projeto aberto em vez de travar", async () => {
    const erro = await retomarDoHistorico({ ...entry, cwd: null }, "p1", {}, deps()).catch((e: unknown) => e) as RetomadaError;
    expect(erro.acoes[0]).toMatchObject({ rotulo: "Abrir no projeto aberto", opcoes: { cwd: "/repo/app" } });
  });

  it("conversa que não existe em conta nenhuma: oferece conversa nova", async () => {
    rotas["/retomada/preparar"] = { status: 200, body: { ...ok, transcript: false, conta: null, pronto: false } };
    const erro = await retomarDoHistorico(entry, "p1", {}, deps()).catch((e: unknown) => e) as RetomadaError;
    expect(erro.acoes[0]).toMatchObject({ opcoes: { cwd: "/repo/app", continuarNova: true } });
  });

  it("continuar numa conversa nova com o histórico: abre pelo engine e foca a aba nova", async () => {
    rotas["/retomada/preparar"] = { status: 200, body: ok };
    rotas["/retomada/continuar-nova"] = { status: 200, body: { conversation_id: "conv-n", terminal_session_id: "term-n", contexto: true } };
    const d = deps();
    await retomarDoHistorico(entry, "p1", { cwd: "/repo/app", continuarNova: true }, d);
    const pedido = chamadas.find((c) => c.path === "/retomada/continuar-nova");
    expect(pedido?.body).toMatchObject({ provider: "claude", session_id: "sess-1", cwd: "/repo/app", project_id: "proj", titulo: "Login" });
    expect(d.dispatch).toHaveBeenCalledWith(expect.objectContaining({ type: "FOCUS_SESSION", sessionId: "term-n" }));
    expect(invoke).not.toHaveBeenCalledWith("spawn_terminal", expect.anything());
  });
});

describe("serviço de retomada", () => {
  it("rota ausente vira null; o erro de pasta sumida é reconhecido e limpo", async () => {
    expect(await prepararRetomada({ provider: "claude", sessionId: "x" })).toBeNull();
    const erro = "Error: PASTA_SUMIU: a pasta /wt/x não existe mais (worktree apagada?).";
    expect(ehPastaSumida(erro)).toBe(true);
    expect(semCodigo(erro)).toBe("A pasta /wt/x não existe mais (worktree apagada?).");
    expect(cliNaoAchouAConversa("No conversation found with session ID: abc")).toBe(true);
    expect(cliNaoAchouAConversa("tudo certo")).toBe(false);
  });

  it("aba de agente que o engine anterior deixou aparece como 'retoma ao abrir'", () => {
    expect(statusDaConversa({ state: "stopped", retoma_ao_abrir: true }, false)).toBe("retoma");
    expect(statusDaConversa({ state: "stopped" }, false)).toBe("neutro");
    expect(statusDaConversa({ state: "working", retoma_ao_abrir: true }, false)).toBe("trabalhando");
  });
});

describe("HistoryView", () => {
  it("erro de retomada mostra as saídas e cada uma tenta de novo com as opções dela", async () => {
    vi.mocked(invoke).mockImplementation(async (comando: string) => {
      if (comando === "agent_history") return [entry];
      if (comando === "agent_history_transcript") return { messages: [], total: 0 };
      if (comando === "list_profiles") return [];
      if (comando === "engine_api") return { type: "api", status: 200, body: "[]" };
      return null;
    });
    const onResume = vi.fn()
      .mockRejectedValueOnce(new RetomadaError("A pasta /repo/app não existe mais.", [{ rotulo: "Abrir no repositório principal", opcoes: { cwd: "/repo" } }]))
      .mockResolvedValueOnce(undefined);
    render(<HistoryView onResume={onResume} />);
    fireEvent.click(await screen.findByText("Login"));
    fireEvent.click(await screen.findByRole("button", { name: "Retomar no terminal" }));
    const saida = await screen.findByRole("button", { name: "Abrir no repositório principal" });
    expect(screen.getByText("A pasta /repo/app não existe mais.")).toBeInTheDocument();
    fireEvent.click(saida);
    await waitFor(() => expect(onResume).toHaveBeenLastCalledWith(entry, "p1", { cwd: "/repo" }));
  });
});

describe("Reiniciar para atualizar", () => {
  it("textos: engine pendente avisa que reinicia e quantos agentes param; só a tela não pede confirmação", () => {
    const so_tela = textosDaAtualizacao({ disponivel: true, reinicia_agentes: false });
    expect(so_tela.engine).toBe(false);
    expect(so_tela.rotulo).toBe("Reiniciar para atualizar");
    const adiado = textosDaAtualizacao({ disponivel: true, reinicia_agentes: false, agentes_ocupados: 2, engine_pendente: true, engine_antigo_rodando: true });
    expect(adiado.rotulo).toBe("Engine desatualizado · atualizar");
    expect(adiado.confirmacao).toContain("O engine também será reiniciado: 2 agentes trabalhando agora serão interrompidos");
    expect(adiado.confirmacao).toContain("--resume");
  });

  it("confirmar troca o engine de fato; 'Só a tela agora' mantém o antigo", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", { configurable: true, value: {} });
    try {
      vi.mocked(invoke).mockImplementation(async (comando: string) => comando === "versao_nova_no_disco"
        ? { disponivel: true, reinicia_agentes: false, agentes_ocupados: 1, engine_pendente: true, engine_antigo_rodando: true }
        : null);
      const { unmount } = render(<AvisoDeAtualizacao />);
      fireEvent.click(await screen.findByRole("button", { name: /Engine desatualizado/ }));
      expect(screen.getByRole("dialog")).toHaveTextContent("1 agente trabalhando agora será interrompido");
      fireEvent.click(screen.getByRole("button", { name: "Reiniciar app e engine" }));
      await waitFor(() => expect(invoke).toHaveBeenCalledWith("reiniciar_para_atualizar", { trocarEngine: true }));
      unmount();

      vi.mocked(invoke).mockClear();
      render(<AvisoDeAtualizacao />);
      fireEvent.click(await screen.findByRole("button", { name: /Engine desatualizado/ }));
      fireEvent.click(screen.getByRole("button", { name: "Só a tela agora" }));
      await waitFor(() => expect(invoke).toHaveBeenCalledWith("reiniciar_para_atualizar", { trocarEngine: false }));
    } finally {
      Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
    }
  });
});
