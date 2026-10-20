import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "./interacoesTelas";
import { invoke } from "@tauri-apps/api/core";
import { AgentSwitcher } from "../features/terminal/AgentSwitcher";
import { clis, perfis } from "./fixturesTelas";

const plano = { conversation_id: "conversa", title: "Login", provider: "claude", profile_id: "trabalho", initial_command: "fake", external_session_id: null, handoff: false, handoff_path: null, notice: null };
function montar() {
  const props = { conversationId: "conversa", currentProvider: "claude" as const, currentProfileId: "pessoal", onCancel: vi.fn(), onSwitch: vi.fn() };
  render(<AgentSwitcher {...props} />);
  return props;
}
beforeEach(() => {
  vi.mocked(invoke).mockReset().mockImplementation(async (cmd) => {
    if (cmd === "agent_cli_statuses") return clis;
    if (cmd === "list_profiles") return perfis;
    if (cmd === "plan_switch") return plano;
    return null;
  });
});

it("oferece outra conta e só as outras IAs disponíveis", async () => {
  montar();
  expect(await screen.findByText("Trabalho")).toBeInTheDocument();
  expect(screen.queryByText("Pessoal")).not.toBeInTheDocument();
  expect(screen.getByText("Codex")).toBeInTheDocument();
  expect(screen.queryByText("Cursor")).not.toBeInTheDocument();
  expect(screen.getByText(/Recomeço com briefing/)).toBeInTheDocument();
});
it("continua na conta escolhida com o plano recebido", async () => {
  const props = montar();
  await userEvent.click(await screen.findByRole("button", { name: "Continuar aqui" }));
  await waitFor(() => expect(props.onSwitch).toHaveBeenCalledWith(plano, clis[0], perfis[1]));
  expect(invoke).toHaveBeenCalledWith("plan_switch", expect.objectContaining({ conversationId: "conversa", targetProvider: "claude", targetProfileId: "trabalho" }));
});
it("passa o bastão para outra IA sem impor uma conta", async () => {
  const props = montar();
  await userEvent.click(await screen.findByRole("button", { name: "Passar o bastão" }));
  await waitFor(() => expect(props.onSwitch).toHaveBeenCalledWith(plano, clis[1], null));
  expect(invoke).toHaveBeenCalledWith("plan_switch", expect.objectContaining({ targetProvider: "codex", targetProfileId: null }));
});
it("mostra falha da troca e permite tentar novamente", async () => {
  vi.mocked(invoke).mockImplementation(async (cmd) => {
    if (cmd === "agent_cli_statuses") return clis;
    if (cmd === "list_profiles") return perfis;
    throw new Error("Não foi possível preparar a conversa");
  });
  const props = montar();
  await userEvent.click(await screen.findByRole("button", { name: "Continuar aqui" }));
  expect(await screen.findByRole("alert")).toHaveTextContent("Não foi possível preparar");
  expect(screen.getByRole("button", { name: "Continuar aqui" })).toBeEnabled();
  expect(props.onSwitch).not.toHaveBeenCalled();
});
it("avisa quando não há outra conta e cancela sem trocar", async () => {
  vi.mocked(invoke).mockResolvedValue([]);
  const props = montar();
  expect(await screen.findByText(/Nenhuma outra conta cadastrada/)).toBeInTheDocument();
  await userEvent.click(screen.getByRole("button", { name: "Cancelar" }));
  expect(props.onCancel).toHaveBeenCalledOnce();
  expect(props.onSwitch).not.toHaveBeenCalled();
});
it("exibe erro ao carregar contas", async () => {
  vi.mocked(invoke).mockRejectedValue(new Error("Contas indisponíveis"));
  montar();
  expect(await screen.findByRole("alert")).toHaveTextContent("Contas indisponíveis");
});
