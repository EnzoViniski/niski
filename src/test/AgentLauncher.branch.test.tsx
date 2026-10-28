import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { AgentLauncher } from "../features/terminal/AgentLauncher";

// Escolher a branch ao abrir o agente. Outra branch roda numa worktree própria.
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

describe("AgentLauncher — branch", () => {
  it("branch que não é a atual prepara a worktree e o agente abre nela", async () => {
    vi.mocked(invoke).mockImplementation(async (command: string) => {
      switch (command) {
        case "agent_cli_statuses":
          return [{ id: "claude", label: "Claude", command: "claude", path: "x", available: true, authenticated: true }];
        case "list_profiles": return [];
        case "git_branches": return [{ name: "main", current: true }, { name: "feat/login", current: false }];
        case "git_worktree_da_branch": return "/p/.omni-agents/worktrees/feat-login";
        default: return null;
      }
    });
    const onLaunch = vi.fn();
    render(<AgentLauncher projectName="p" projectPath="/p" onLaunch={onLaunch} />);
    await screen.findByText("Branch");
    fireEvent.click(await screen.findByRole("combobox", { name: /Branch/ }));
    fireEvent.click(await screen.findByRole("option", { name: /feat\/login/ }));
    expect(await screen.findByText(/cópia da pasta só dele/)).toBeInTheDocument();
    const abrir = screen.getByRole("button", { name: /Abrir Claude/ });
    await waitFor(() => expect(abrir).not.toBeDisabled());
    fireEvent.click(abrir);
    await waitFor(() => expect(onLaunch).toHaveBeenCalledWith(expect.objectContaining({ cwd: "/p/.omni-agents/worktrees/feat-login" })));
    expect(invoke).toHaveBeenCalledWith("git_worktree_da_branch", { projectPath: "/p", branch: "feat/login", criar: false });
  });
});
