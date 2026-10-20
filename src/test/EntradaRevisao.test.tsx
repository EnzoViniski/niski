import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { AbaDeAgente } from "../features/conversa/AbaDeAgente";
import { usarTransporte } from "../mobile/api";
import { listTerminalSessions } from "../features/terminal/terminalService";

vi.mock("../features/terminal/terminalService", async () => ({
  ...await vi.importActual<typeof import("../features/terminal/terminalService")>("../features/terminal/terminalService"), listTerminalSessions: vi.fn(),
}));

beforeEach(() => {
  vi.mocked(listTerminalSessions).mockResolvedValue([{ id: "s", conversation_id: "c" } as never]);
  usarTransporte(async () => new Response(JSON.stringify({ timeline: { messages: [], next_cursor: null, unavailable_segments: [] }, actions: [], conversation: { id: "c", title: "Agente", provider: "codex", capabilities: { prompt: true, revision: "r" } } })));
});
afterEach(() => usarTransporte((p, i) => fetch(p, i)));

it.each(["conversa", "terminal"] as const)("abre revisão pela faixa no modo %s", async modo => {
  const abrir = vi.fn();
  render(<AbaDeAgente agentView={modo} sessionId="s" onRevisao={abrir}><div>Terminal sintético</div></AbaDeAgente>);
  await act(async () => { await userEvent.click(await screen.findByRole("button", { name: "Revisão" })); });
  expect(abrir).toHaveBeenCalledTimes(1);
});
