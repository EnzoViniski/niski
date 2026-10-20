import { useReducer } from "react";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "./interacoesTelas";
import { invoke } from "@tauri-apps/api/core";
import { KanbanBoard } from "../features/kanban/KanbanBoard";
import { emptyKanban, kanbanReducer } from "../features/kanban/kanbanReducer";
import { KANBAN_CARD_DRAG_TYPE } from "../features/kanban/kanbanDrag";
import type { KanbanState } from "../types/kanban";
import { clis, tarefa } from "./fixturesTelas";

function montar(overrides: Partial<KanbanState> = {}) {
  function Quadro() {
    const [kanban, dispatch] = useReducer(kanbanReducer, { ...emptyKanban(), ...overrides });
    return <KanbanBoard projectId="proj" kanban={kanban} dispatch={dispatch} />;
  }
  render(<Quadro />);
}
beforeEach(() => { vi.mocked(invoke).mockReset().mockResolvedValue(clis); });

it("separa as tarefas por projeto e por coluna", async () => {
  montar({ tasks: [tarefa, { ...tarefa, id: "outra", projectId: "outro", title: "Não aparece" }] });
  expect(screen.getByText("Pronta (1)")).toBeInTheDocument();
  expect(screen.getByText("Backlog (0)")).toBeInTheDocument();
  expect(screen.queryByText("Não aparece")).not.toBeInTheDocument();
  await screen.findByRole("option", { name: "Claude" });
});
it("cria uma tarefa com descrição e repetição e fecha o formulário", async () => {
  montar();
  await userEvent.click(screen.getByRole("button", { name: "+ Nova task" }));
  expect(screen.getByRole("button", { name: "Salvar" })).toBeDisabled();
  await userEvent.type(screen.getByPlaceholderText("Título da task"), "  Revisar login  ");
  await userEvent.type(screen.getByPlaceholderText(/Descrição/), "Validar o foco");
  await userEvent.type(screen.getByRole("spinbutton"), "3");
  await userEvent.click(screen.getByRole("button", { name: "Salvar" }));
  expect(screen.getByText("Revisar login")).toBeInTheDocument();
  expect(screen.getByText("Validar o foco")).toBeInTheDocument();
  expect(screen.getByText(/1\/3/)).toBeInTheDocument();
  expect(screen.getByText("Backlog (1)")).toBeInTheDocument();
  expect(screen.queryByPlaceholderText("Título da task")).not.toBeInTheDocument();
});
it("edita uma tarefa existente e depois a exclui", async () => {
  montar({ tasks: [tarefa] });
  await userEvent.click(screen.getByRole("button", { name: "Editar task" }));
  const campo = screen.getByPlaceholderText("Título da task");
  await userEvent.clear(campo);
  await userEvent.type(campo, "Revisar cadastro");
  await userEvent.click(screen.getByRole("button", { name: "Salvar" }));
  expect(screen.getByText("Revisar cadastro")).toBeInTheDocument();
  expect(screen.queryByText(tarefa.title)).not.toBeInTheDocument();
  await userEvent.click(screen.getByRole("button", { name: "Excluir task" }));
  expect(screen.queryByText("Revisar cadastro")).not.toBeInTheDocument();
  expect(screen.getByText("Pronta (0)")).toBeInTheDocument();
});
it("cancela sem criar uma tarefa", async () => {
  montar();
  await userEvent.click(screen.getByRole("button", { name: "+ Nova task" }));
  await userEvent.type(screen.getByPlaceholderText("Título da task"), "Rascunho");
  await userEvent.click(screen.getByRole("button", { name: "Cancelar" }));
  expect(screen.queryByText("Rascunho")).not.toBeInTheDocument();
  expect(screen.getByText("Backlog (0)")).toBeInTheDocument();
});
it("liga/desliga o dispatcher e escolhe uma CLI disponível", async () => {
  montar();
  await userEvent.click(screen.getByRole("button", { name: /Dispatcher automático: desligado/ }));
  expect(screen.getByRole("button", { name: /Dispatcher automático: ligado/ })).toBeInTheDocument();
  await screen.findByRole("option", { name: "Codex" });
  expect(screen.getByRole("option", { name: /Cursor/ })).toBeDisabled();
  await userEvent.selectOptions(screen.getByRole("combobox", { name: "Agente" }), "codex");
  expect(screen.getByRole("combobox")).toHaveValue("codex");
  await userEvent.click(screen.getByRole("button", { name: /Dispatcher automático: ligado/ }));
  expect(screen.getByRole("button", { name: /Dispatcher automático: desligado/ })).toBeInTheDocument();
});
it("move o cartão para a coluna em que foi solto e ignora outro arraste", async () => {
  montar({ tasks: [tarefa] });
  const coluna = screen.getByText("Concluída (0)").parentElement!;
  fireEvent.drop(coluna, { dataTransfer: { getData: (tipo: string) => tipo === KANBAN_CARD_DRAG_TYPE ? JSON.stringify({ taskId: tarefa.id }) : "" } });
  expect(screen.getByText("Concluída (1)")).toBeInTheDocument();
  fireEvent.drop(screen.getByText("Backlog (0)").parentElement!, { dataTransfer: { getData: () => "" } });
  expect(screen.getByText("Backlog (0)")).toBeInTheDocument();
  await waitFor(() => expect(screen.getByText(tarefa.title)).toBeInTheDocument());
});
it("mostra que o dispatcher foi pausado por falhas", async () => {
  montar({ pausedUntil: new Date(Date.now() + 60_000).toISOString() });
  expect(screen.getByText(/Pausado por falhas até/)).toBeInTheDocument();
  await screen.findByRole("option", { name: "Claude" });
});
