import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import { AccountUsage } from "../features/terminal/AccountUsage";

it("mostra origem Codex e ausência semanal sem transformar em zero", async () => {
  vi.mocked(invoke).mockResolvedValueOnce({ status: "available", primary: { used_percent: 5, resets_at: 2000000000 }, secondary: null });
  render(<AccountUsage profileId="p" provider="codex" />);
  expect(await screen.findByText("5% consumido")).toBeInTheDocument();
  expect(screen.getByText("Medido no registro do Codex")).toBeInTheDocument();
  expect(screen.queryByText("0% consumido")).not.toBeInTheDocument();
});

// Fork (harness, C5): o uso do Claude vem da statusline; o botão só relê (nada é digitado).
it("Claude relê o uso no clique e informa a origem", async () => {
  vi.mocked(invoke).mockClear().mockResolvedValue({ status: "unknown", primary: null, secondary: null, reason: "Nenhuma sessão" });
  render(<AccountUsage profileId="claude" provider="claude" />);
  await waitFor(() => expect(invoke).toHaveBeenCalledWith("account_usage", { profileId: "claude", refresh: false }));
  expect(screen.getByText("Última statusline do Claude Code")).toBeInTheDocument();
  await userEvent.click(screen.getByRole("button", { name: "Atualizar uso" }));
  expect(invoke).toHaveBeenLastCalledWith("account_usage", { profileId: "claude", refresh: true });
  expect(screen.queryByText("0% consumido")).not.toBeInTheDocument();
});
