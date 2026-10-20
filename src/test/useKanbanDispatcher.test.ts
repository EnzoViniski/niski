import { act, renderHook } from "@testing-library/react";
import { useKanbanDispatcher } from "../features/kanban/useKanbanDispatcher";
import { emptyKanban } from "../features/kanban/kanbanReducer";
import type { KanbanState } from "../types/kanban";
import type { Project } from "../types/workspace";
import { ensureAgentTrust, listAgentClis, spawnTerminal, terminalSnapshot, writeTerminal, type TerminalSession } from "../features/terminal/terminalService";
import { clis, sessao, tarefa } from "./fixturesTelas";

vi.mock("../features/terminal/terminalService", () => ({
  ensureAgentTrust: vi.fn(), listAgentClis: vi.fn(), spawnTerminal: vi.fn(), terminalSnapshot: vi.fn(), writeTerminal: vi.fn(),
}));
const projeto: Project = { id: "proj", name: "Projeto", path: "/fake/projeto", gitRoot: null, branch: null, composeFile: null, lastOpenedAt: "", layout: { type: "pane", id: "pane", tabs: [], activeTabId: null }, activePaneId: "pane", maximizedPaneId: null };
function montar(parte: Partial<KanbanState> = {}) {
  const dispatch = vi.fn();
  const kanban = { ...emptyKanban(), dispatcherEnabled: true, tasks: [{ ...tarefa }], ...parte };
  const hook = renderHook(({ estado, sessoes }: { estado: KanbanState; sessoes: TerminalSession[] }) => useKanbanDispatcher(estado, dispatch, [projeto], sessoes), { initialProps: { estado: kanban, sessoes: [sessao] } });
  return { ...hook, dispatch, kanban };
}
const passar = (ms: number) => act(async () => { await vi.advanceTimersByTimeAsync(ms); });
beforeEach(() => {
  vi.useFakeTimers();
  vi.mocked(listAgentClis).mockReset().mockResolvedValue(clis);
  vi.mocked(ensureAgentTrust).mockReset().mockResolvedValue(undefined);
  vi.mocked(spawnTerminal).mockReset().mockResolvedValue(sessao);
  vi.mocked(writeTerminal).mockReset().mockResolvedValue(undefined);
  vi.mocked(terminalSnapshot).mockReset().mockResolvedValue({ data: "Revisão concluída", from_seq: 0, next_seq: 1, session: sessao });
});
afterEach(() => vi.useRealTimers());

it.each([
  { dispatcherEnabled: false },
  { pausedUntil: new Date(Date.now() + 60_000).toISOString() },
])("não envia tarefas quando desligado ou pausado: %j", async (parte) => {
  montar(parte);
  await passar(5000);
  expect(spawnTerminal).not.toHaveBeenCalled();
  expect(writeTerminal).not.toHaveBeenCalled();
});
it("usa a CLI configurada e só envia a tarefa após a inicialização", async () => {
  const { dispatch } = montar({ agentByProject: { proj: "codex" } });
  await passar(0);
  expect(ensureAgentTrust).toHaveBeenCalledWith("codex", projeto.path);
  expect(spawnTerminal).toHaveBeenCalledWith(expect.objectContaining({ projectId: "proj", provider: "codex", initialCommand: "codex" }));
  expect(writeTerminal).not.toHaveBeenCalled();
  await passar(1500);
  expect(writeTerminal).toHaveBeenCalledWith("sessao", `${tarefa.title}\n\n${tarefa.description}\r`);
  expect(dispatch).toHaveBeenCalledWith({ type: "TASK_STARTED", id: "task" });
});
it("escolhe a primeira disponível quando a configurada não existe", async () => {
  montar({ agentByProject: { proj: "cursor" } });
  await passar(1500);
  expect(spawnTerminal).toHaveBeenCalledWith(expect.objectContaining({ provider: "claude" }));
});
it("aguarda se nenhuma CLI está disponível", async () => {
  vi.mocked(listAgentClis).mockResolvedValue(clis.map((a) => ({ ...a, available: false })));
  const { dispatch } = montar();
  await passar(5000);
  expect(spawnTerminal).not.toHaveBeenCalled();
  expect(dispatch).not.toHaveBeenCalled();
});
it("respeita a posição da fila e ignora tarefas de outro projeto", async () => {
  montar({ tasks: [{ ...tarefa, id: "depois", position: 8, title: "Depois" }, { ...tarefa, id: "outro", projectId: "outro", position: -1 }, tarefa] });
  await passar(1500);
  expect(writeTerminal).toHaveBeenCalledOnce();
  expect(writeTerminal).toHaveBeenCalledWith("sessao", expect.stringContaining(tarefa.title));
});
it("conclui com o resultado e reutiliza a sessão na próxima tarefa", async () => {
  const { dispatch, kanban, rerender } = montar();
  await passar(1500);
  rerender({ estado: { ...kanban, tasks: [{ ...tarefa, status: "doing" }] }, sessoes: [{ ...sessao, state: "answered" }] });
  await passar(1000);
  expect(dispatch).toHaveBeenCalledWith({ type: "TASK_FINISHED", id: "task", outcome: "done", resultNote: "Revisão concluída" });
  rerender({ estado: { ...kanban, tasks: [{ ...tarefa, id: "proxima", title: "Próxima" }] }, sessoes: [{ ...sessao, state: "answered" }] });
  await passar(1000);
  expect(spawnTerminal).toHaveBeenCalledOnce();
  expect(writeTerminal).toHaveBeenLastCalledWith("sessao", expect.stringContaining("Próxima"));
});
it("não conclui nem aprova uma tarefa enquanto a CLI espera permissão", async () => {
  const { dispatch, kanban, rerender } = montar();
  await passar(1500);
  dispatch.mockClear();
  rerender({ estado: { ...kanban, tasks: [{ ...tarefa, status: "doing" }] }, sessoes: [{ ...sessao, state: "approval_required" }] });
  await passar(3000);
  expect(dispatch).not.toHaveBeenCalled();
  expect(writeTerminal).toHaveBeenCalledOnce();
});
it.each(["crashed", "stopped", "orphan", "ausente"] as const)("marca falha e alimenta a pausa por falhas se a sessão fica %s", async (estado) => {
  const { dispatch, kanban, rerender } = montar();
  await passar(1500);
  rerender({ estado: { ...kanban, tasks: [{ ...tarefa, status: "doing" }] }, sessoes: estado === "ausente" ? [] : [{ ...sessao, state: estado }] });
  await passar(1000);
  expect(dispatch).toHaveBeenCalledWith(expect.objectContaining({ type: "TASK_FINISHED", id: "task", outcome: "failed" }));
  expect(dispatch).toHaveBeenCalledWith({ type: "RECORD_FAILURE" });
});
it("remove a pausa vencida e volta a despachar", async () => {
  const { dispatch } = montar({ pausedUntil: new Date(Date.now() - 1000).toISOString() });
  await passar(1500);
  expect(dispatch).toHaveBeenCalledWith({ type: "CLEAR_PAUSE" });
  expect(writeTerminal).toHaveBeenCalledOnce();
});
it("conclui mesmo se a captura do resultado não estiver disponível", async () => {
  vi.mocked(terminalSnapshot).mockRejectedValue(new Error("Snapshot indisponível"));
  const { dispatch, kanban, rerender } = montar();
  await passar(1500);
  rerender({ estado: { ...kanban, tasks: [{ ...tarefa, status: "doing" }] }, sessoes: [{ ...sessao, state: "answered" }] });
  await passar(1000);
  expect(dispatch).toHaveBeenCalledWith({ type: "TASK_FINISHED", id: "task", outcome: "done", resultNote: "" });
});
// Bug: o delay de inicialização continua depois do cleanup (useKanbanDispatcher.ts:118–121).
it.fails("não envia o prompt depois de desmontar durante a inicialização", async () => {
  const { unmount } = montar();
  await passar(0);
  expect(spawnTerminal).toHaveBeenCalledOnce();
  unmount();
  await passar(1500);
  expect(writeTerminal).not.toHaveBeenCalled();
});
