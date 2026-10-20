import { createElement } from "react";
import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { PastasDeProjetos } from "../features/projects/PastasDeProjetos";
import { anterioresDaPasta, redefinirConversasAnteriores, titulosDoIndice } from "../features/projects/historicoDaPasta";
import type { HistoryEntry } from "../features/history/historyService";
import type { Conversation, TerminalSession } from "../features/terminal/terminalService";
import type { Project } from "../types/workspace";

/*
 * Fork (histórico na lateral): conversa fechada (ou de antes do app) não some da pasta do projeto —
 * aparece em "Anteriores", lida dos transcripts das CLIs, e um clique retoma.
 */

function entrada(parcial: Partial<HistoryEntry>): HistoryEntry {
  return {
    provider: "claude", profile_id: "claude-padrao", profile_name: "Padrão", session_id: "s1",
    title: null, first_prompt: "oi", cwd: "/proj", started_at_ms: 1, updated_at_ms: 1, path: "/x.jsonl",
    ...parcial,
  };
}

function sessao(parcial: Partial<TerminalSession>): TerminalSession {
  return {
    id: "t1", project_id: "p1", name: "Claude", cwd: "/proj", shell: "zsh", state: "idle", pid: 1,
    output_seq: 0, rows: 24, cols: 80, provider: "claude", ...parcial,
  } as TerminalSession;
}

describe("anterioresDaPasta", () => {
  it("lista só a pasta, mais recentes primeiro, sem as abertas, vazias ou repetidas", () => {
    const entradas = [
      entrada({ session_id: "velha", updated_at_ms: 10, first_prompt: "primeira" }),
      entrada({ session_id: "nova", updated_at_ms: 30, first_prompt: "segunda" }),
      entrada({ session_id: "nova", profile_id: "outra", updated_at_ms: 20 }),
      entrada({ session_id: "aberta", updated_at_ms: 40 }),
      entrada({ session_id: "vazia", updated_at_ms: 50, first_prompt: null, title: null }),
      entrada({ session_id: "fora", cwd: "/outro", updated_at_ms: 60 }),
      entrada({ session_id: "codex", provider: "codex", cwd: "/proj/", updated_at_ms: 25, title: "Codex título" }),
    ];
    const lista = anterioresDaPasta("/proj", { entradas, titulos: { velha: "Renomeada no app" } },
      [sessao({ external_session_id: "aberta" })]);
    expect(lista.map((item) => [item.entry.session_id, item.titulo])).toEqual([
      ["nova", "segunda"],
      ["codex", "Codex título"],
      ["velha", "Renomeada no app"],
    ]);
  });

  it("título do índice vem de qualquer segmento da conversa", () => {
    const conversa = {
      id: "c1", project_id: "p1", cwd: "/proj", title: "Plano", title_source: "manual", created_at_ms: 1,
      segments: [{ provider: "claude", profile_id: null, external_session_id: "a", transcript_path: null, terminal_session_id: null, started_at_ms: 1, ended_at_ms: 2 },
        { provider: "codex", profile_id: null, external_session_id: "b", transcript_path: null, terminal_session_id: null, started_at_ms: 2, ended_at_ms: null }],
    } as Conversation;
    expect(titulosDoIndice([conversa])).toEqual({ a: "Plano", b: "Plano" });
  });
});

describe("Pastas na lateral", () => {
  const projeto = { id: "p1", name: "proj", path: "/proj" } as Project;

  beforeEach(() => {
    localStorage.clear();
    localStorage.setItem("omni-pastas-abertas", JSON.stringify(["p1"]));
    vi.mocked(invoke).mockReset();
    vi.mocked(invoke).mockImplementation(async (comando: string, args?: unknown) => {
      if (comando === "agent_history") {
        return (args as { provider: string }).provider === "claude"
          ? [entrada({ session_id: "antiga", first_prompt: "conversa de ontem", updated_at_ms: Date.now() - 86_400_000 })]
          : [];
      }
      return [];
    });
    redefinirConversasAnteriores();
  });

  function desenhar(onRetomarAnterior = vi.fn()) {
    render(createElement(PastasDeProjetos, {
      projects: [projeto], activeProjectId: "p1", activeSessionId: null, agentSessions: [], naoVistas: new Set(),
      onSelectProject: vi.fn(), onCloseProject: vi.fn(), onAbrirConversa: vi.fn(), onBifurcar: vi.fn(),
      onReiniciar: vi.fn(), onFechar: vi.fn(), onRetomarAnterior, onVerHistorico: vi.fn(),
    }));
    return onRetomarAnterior;
  }

  it("mostra a conversa antiga da pasta e retoma no clique", async () => {
    const retomar = desenhar();
    const botao = await screen.findByRole("button", { name: /Retomar conversa anterior conversa de ontem/ });
    expect(screen.getByText("Anteriores")).toBeTruthy();
    await act(async () => { fireEvent.click(botao); });
    expect(retomar).toHaveBeenCalledWith(expect.objectContaining({ session_id: "antiga", provider: "claude" }));
  });

  it("erro ao retomar aparece na pasta", async () => {
    desenhar(vi.fn().mockRejectedValue(new Error("Claude não foi encontrado no PATH.")));
    fireEvent.click(await screen.findByRole("button", { name: /Retomar conversa anterior/ }));
    await waitFor(() => expect(screen.getByRole("alert").textContent).toContain("PATH"));
  });
});
