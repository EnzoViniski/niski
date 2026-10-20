import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { AgentLauncher } from "../features/terminal/AgentLauncher";

it("abre Codex com servidor e modelo locais mesmo sem login em conta online", async () => {
  vi.mocked(invoke).mockReset().mockImplementation(async (comando) => {
    if (comando === "agent_cli_statuses") return [{ id: "codex", label: "Codex", command: "codex", available: true, authenticated: false, path: "/fake/codex" }];
    if (comando === "list_profiles") return [{ id: "padrao", provider: "codex", name: "Padrão", builtin: true, authenticated: false, config_dir: "/fake/.codex" }];
    if (comando === "git_branches") return [];
    return null;
  });
  const onLaunch = vi.fn();
  render(<AgentLauncher projectName="Teste" projectPath="/fake/projeto" onLaunch={onLaunch} />);
  fireEvent.click(await screen.findByRole("combobox", { name: "Usar modelo" }));
  fireEvent.click(await screen.findByRole("option", { name: "Local com Ollama" }));
  const abrir = screen.getByRole("button", { name: "Abrir Codex" });
  expect(abrir).toBeDisabled();
  expect(screen.queryByText(/ainda não tem login/)).not.toBeInTheDocument();
  fireEvent.change(screen.getByLabelText("Nome do modelo instalado"), { target: { value: "empresa/modelo:local" } });
  await waitFor(() => expect(abrir).not.toBeDisabled());
  fireEvent.click(abrir);
  expect(onLaunch).toHaveBeenCalledWith(expect.objectContaining({
    agent: expect.objectContaining({ command: "codex --oss --local-provider ollama --model empresa/modelo:local" }),
    profile: expect.objectContaining({ id: "padrao" }),
  }));
});
