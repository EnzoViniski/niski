import { invoke } from "@tauri-apps/api/core";
import { usarTransporte } from "../mobile/api";
import { listTerminalSessions, type TerminalSession } from "../features/terminal/terminalService";
import { aplicarRevisao, enviarComentarios, listarRevisao, type Arquivo } from "../features/revisao/revisaoService";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("../features/terminal/terminalService", () => ({ listTerminalSessions: vi.fn() }));

const transporte = vi.fn();
beforeEach(() => {
  vi.clearAllMocks();
  usarTransporte(transporte);
  vi.mocked(listTerminalSessions).mockResolvedValue([{ id: "s", conversation_id: "conversa/atual" } as TerminalSession]);
  transporte.mockImplementation(async (_path: string, init: RequestInit) => new Response(JSON.stringify(init.method === "POST" ? { id: "acao" } : { conversation: { capabilities: { prompt: true, revision: "r1" } } }), { status: 200 }));
});
afterEach(() => usarTransporte((p, i) => fetch(p, i)));

it("reutiliza invoke e transmite patch esperado e índice do trecho", async () => {
  await listarRevisao("/p", false);
  expect(invoke).toHaveBeenCalledWith("revisao_listar", { projectPath: "/p", staged: false });
  const a = { caminho: "a", staged: true, patch: "patch" } as Arquivo;
  await aplicarRevisao("/p", a, "unstage", 2);
  expect(invoke).toHaveBeenLastCalledWith("revisao_aplicar", { projectPath: "/p", caminho: "a", staged: true, esperado: "patch", acao: "unstage", trecho: 2 });
});

it("envia pela mesma rota de prompt com revisão e idempotência", async () => {
  await enviarComentarios("s", "comentários", "chave");
  expect(transporte).toHaveBeenCalledWith("/conversas/conversa%2Fatual/prompt", expect.objectContaining({
    method: "POST", headers: expect.objectContaining({ "If-Match": "r1", "Idempotency-Key": "chave" }), body: JSON.stringify({ texto: "comentários" }),
  }));
});

it("usa a fila do compositor quando o agente trabalha", async () => {
  transporte.mockResolvedValueOnce(new Response(JSON.stringify({ conversation: { capabilities: { prompt: false, enfileirar: true, revision: "r2" } } })));
  await enviarComentarios("s", "comentários", "chave");
  expect(transporte).toHaveBeenLastCalledWith("/conversas/conversa%2Fatual/prompt", expect.objectContaining({ body: JSON.stringify({ texto: "comentários", fila: true }) }));
});

it("preserva pedido de aprovação e não inventa envio por PTY", async () => {
  transporte.mockResolvedValueOnce(new Response(JSON.stringify({ conversation: { capabilities: { prompt: false, approve: true } } })));
  await expect(enviarComentarios("s", "comentários")).rejects.toThrow("pedido de permissão");
  expect(transporte).toHaveBeenCalledTimes(1);
});

it("recusa sessão sem conversa registrada", async () => {
  vi.mocked(listTerminalSessions).mockResolvedValue([]);
  await expect(enviarComentarios("s", "comentários")).rejects.toThrow("conversa ativa");
  expect(transporte).not.toHaveBeenCalled();
});
