import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { AgentLauncher } from "../features/terminal/AgentLauncher";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const entry = {
  provider: "claude",
  profile_id: "p2",
  profile_name: "Trabalho",
  session_id: "sess-42",
  title: "Conserte o login",
  first_prompt: null,
  cwd: "C:/proj",
  started_at_ms: 1,
  updated_at_ms: 2,
  path: "C:/t.jsonl",
};

describe("AgentLauncher — retomar conversa", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockImplementation(async (command: string) => {
      switch (command) {
        case "agent_cli_statuses":
          return [{ id: "claude", label: "Claude", command: "claude", path: "x", available: true, authenticated: true }];
        case "list_profiles":
          return [
            { id: "p1", name: "Pessoal", provider: "claude", authenticated: true },
            { id: "p2", name: "Trabalho", provider: "claude", authenticated: true },
          ];
        case "agent_history":
          return [entry, { ...entry, session_id: "outro", cwd: "C:/outro", path: "C:/o.jsonl", title: "De outro projeto" }];
        case "agent_history_transcript":
          return { messages: [{ role: "user", text: "mensagem antiga", timestamp: null }], total: 1 };
        case "engine_api":
          return { type: "api", status: 200, body: "{\"ok\":true}" };
        default:
          return null;
      }
    });
  });

  it("abre o histórico do projeto, mostra as mensagens e retoma na conta da conversa", async () => {
    const onLaunch = vi.fn();
    render(<AgentLauncher projectName="proj" projectPath="c:/proj/" onLaunch={onLaunch} />);
    const retomar = await screen.findByRole("button", { name: "Retomar conversa" });
    await waitFor(() => expect(retomar).not.toBeDisabled());
    fireEvent.click(retomar);

    fireEvent.click(await screen.findByText("Conserte o login"));
    expect(screen.queryByText("De outro projeto")).toBeNull();
    expect(await screen.findByText("mensagem antiga")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Retomar no terminal" }));
    await waitFor(() => expect(onLaunch).toHaveBeenCalledWith(
      expect.objectContaining({
        agent: expect.objectContaining({ command: "claude --resume sess-42" }),
        profile: expect.objectContaining({ id: "p2" }),
      })
    ));
    // Mesma conta da conversa: nada a copiar.
    expect(invoke).not.toHaveBeenCalledWith("engine_api", expect.anything());
  });

  it("retoma em outra conta escolhida, levando a conversa para ela antes", async () => {
    const onLaunch = vi.fn();
    render(<AgentLauncher projectName="proj" projectPath="c:/proj/" onLaunch={onLaunch} />);
    const retomar = await screen.findByRole("button", { name: "Retomar conversa" });
    await waitFor(() => expect(retomar).not.toBeDisabled());
    fireEvent.click(retomar);
    fireEvent.click(await screen.findByText("Conserte o login"));

    fireEvent.click(await screen.findByRole("combobox", { name: /abrir na conta/i }));
    fireEvent.click(await screen.findByRole("option", { name: /Pessoal/ }));
    expect(screen.getByText(/copiada para essa conta/i)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Retomar no terminal" }));

    await waitFor(() => expect(onLaunch).toHaveBeenCalledWith(
      expect.objectContaining({ profile: expect.objectContaining({ id: "p1" }) })
    ));
    expect(invoke).toHaveBeenCalledWith("engine_api", expect.objectContaining({
      method: "POST",
      path: "/historico/levar",
      body: JSON.stringify({ provider: "claude", profile_id: "p2", session_id: "sess-42", destino: "p1" }),
    }));
  });
});
