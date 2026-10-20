import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { RevisaoPane } from "../features/revisao/RevisaoPane";
import * as service from "../features/revisao/revisaoService";
import type { Arquivo } from "../features/revisao/revisaoService";
import { SessionProvidersContext } from "../features/terminal/sessionProviders";
import type { TerminalSession } from "../features/terminal/terminalService";

vi.mock("../features/revisao/realce", () => ({ realcar: vi.fn(async (_: string, linhas: string[]) => linhas.map(texto => [{ texto, tipo: "texto" }])) }));
vi.mock("../features/revisao/revisaoService", async () => ({ ...await vi.importActual<typeof service>("../features/revisao/revisaoService"), listarRevisao: vi.fn(), aplicarRevisao: vi.fn(), enviarComentarios: vi.fn() }));

// RTL 14/React 18: mantém as atualizações assíncronas dentro do act explícito.
const usuario = {
  click: async (elemento: Element) => { await act(async () => { await userEvent.click(elemento); }); },
  type: async (elemento: Element, texto: string) => { await act(async () => { await userEvent.type(elemento, texto); }); },
  clear: async (elemento: Element) => { await act(async () => { await userEvent.clear(elemento); }); },
  selectOptions: async (elemento: Element, valor: string) => { await act(async () => { await userEvent.selectOptions(elemento, valor); }); },
};

const arquivo: Arquivo = {
  caminho: "src/exemplo.ts", staged: false, status: "Modificado", adicionadas: 1, removidas: 1,
  patch: "patch sintético", granular: true,
  trechos: [{ cabecalho: "@@ -1 +1 @@", patch: "trecho sintético", linhas: [
    { tipo: "-", antiga: 1, nova: null, texto: "const antes = 1;" },
    { tipo: "+", antiga: null, nova: 1, texto: "const depois = 2;" },
  ] }],
};

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(service.listarRevisao).mockResolvedValue([arquivo]);
  vi.mocked(service.aplicarRevisao).mockResolvedValue();
  vi.mocked(service.enviarComentarios).mockResolvedValue();
});

function montar() {
  const sessao = { id: "sessao", cwd: "/projeto/worktree", name: "Agente atual" } as TerminalSession;
  return render(<SessionProvidersContext.Provider value={{ sessao }}><RevisaoPane projectPath="/projeto" sessionId="sessao" /></SessionProvidersContext.Provider>);
}

it("lista contagens, status, escopos e usa a worktree da sessão", async () => {
  montar();
  expect(await screen.findByText("+1")).toBeInTheDocument();
  expect(screen.getByText("−1")).toBeInTheDocument();
  expect(screen.getByText("Modificado · Fora de staged")).toBeInTheDocument();
  expect(service.listarRevisao).toHaveBeenCalledWith("/projeto/worktree", false);
  await usuario.selectOptions(screen.getByLabelText("Escopo da revisão"), "staged");
  await waitFor(() => expect(service.listarRevisao).toHaveBeenLastCalledWith("/projeto/worktree", true));
});

it("prepara trecho e arquivo, enviando o snapshot completo para validação", async () => {
  montar();
  await usuario.click(await screen.findByRole("button", { name: "Stage do trecho" }));
  expect(service.aplicarRevisao).toHaveBeenCalledWith("/projeto/worktree", arquivo, "stage", 0);
  await usuario.click(screen.getByRole("button", { name: "Stage do arquivo" }));
  expect(service.aplicarRevisao).toHaveBeenLastCalledWith("/projeto/worktree", arquivo, "stage", null);
});

it("oferece unstage nas duas ações da camada staged", async () => {
  const preparado = { ...arquivo, staged: true };
  vi.mocked(service.listarRevisao).mockResolvedValue([preparado]);
  montar();
  await usuario.click(await screen.findByRole("button", { name: "Unstage do trecho" }));
  expect(service.aplicarRevisao).toHaveBeenLastCalledWith("/projeto/worktree", preparado, "unstage", 0);
  await usuario.click(screen.getByRole("button", { name: "Unstage do arquivo" }));
  expect(service.aplicarRevisao).toHaveBeenLastCalledWith("/projeto/worktree", preparado, "unstage", null);
});

it.each([["Descartar trecho", 0], ["Descartar arquivo", null]] as const)("%s só executa após confirmação explícita", async (botao, trecho) => {
  montar();
  await usuario.click(await screen.findByRole("button", { name: botao }));
  expect(service.aplicarRevisao).not.toHaveBeenCalled();
  let dialogo = screen.getByRole("alertdialog", { name: "Confirmar descarte" });
  await usuario.click(within(dialogo).getByRole("button", { name: "Cancelar" }));
  expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
  expect(service.aplicarRevisao).not.toHaveBeenCalled();
  await usuario.click(screen.getByRole("button", { name: botao }));
  dialogo = screen.getByRole("alertdialog");
  await usuario.click(within(dialogo).getByRole("button", { name: "Confirmar descarte" }));
  expect(service.aplicarRevisao).toHaveBeenCalledWith("/projeto/worktree", arquivo, "descartar", trecho);
});

async function comentar(rotulo: RegExp, texto: string) {
  await usuario.click(await screen.findByRole("button", { name: rotulo }));
  await usuario.type(screen.getByRole("textbox"), texto);
  await usuario.click(screen.getByRole("button", { name: "Salvar comentário" }));
}

it("agrega comentários das duas versões com arquivo:linha e trecho, e os envia à sessão de origem", async () => {
  montar();
  await comentar(/Comentar linha 1 anterior/, "Preservar a validação");
  await comentar(/Comentar linha 1 atual/, "Usar constante");
  await usuario.click(screen.getByRole("button", { name: "Enviar comentários ao agente (2)" }));
  const [sessao, texto, chave] = vi.mocked(service.enviarComentarios).mock.calls[0];
  expect(sessao).toBe("sessao");
  expect(texto).toContain("src/exemplo.ts:1 (versão anterior)");
  expect(texto).toContain("src/exemplo.ts:1 (versão atual)");
  expect(texto).toContain("@@ -1 +1 @@\n-const antes = 1;\n+const depois = 2;");
  expect(texto).toContain("Preservar a validação");
  expect(texto).toContain("Usar constante");
  expect(chave).toBeTruthy();
  expect(await screen.findByRole("status")).toHaveTextContent("Comentários enviados");
  expect(screen.getByRole("button", { name: "Enviar comentários ao agente (0)" })).toBeDisabled();
});

it("preserva comentários e chave de idempotência quando o envio falha", async () => {
  vi.mocked(service.enviarComentarios).mockRejectedValueOnce(new Error("Sem conexão"));
  montar();
  await comentar(/Comentar linha 1 atual/, "Revisar");
  await usuario.click(screen.getByRole("button", { name: "Enviar comentários ao agente (1)" }));
  expect(await screen.findByRole("alert")).toHaveTextContent("Sem conexão");
  const primeiro = vi.mocked(service.enviarComentarios).mock.calls[0];
  await usuario.click(screen.getByRole("button", { name: "Enviar comentários ao agente (1)" }));
  expect(vi.mocked(service.enviarComentarios).mock.calls[1]).toEqual(primeiro);
});

it("edita e remove comentários; mantém snapshots ao atualizar o diff", async () => {
  montar();
  await comentar(/Comentar linha 1 atual/, "Primeiro");
  await usuario.click(screen.getByRole("button", { name: "Atualizar" }));
  expect(await screen.findByRole("button", { name: /src\/exemplo.ts:1 — Primeiro/ })).toBeInTheDocument();
  await usuario.click(screen.getByRole("button", { name: /src\/exemplo.ts:1 — Primeiro/ }));
  await usuario.clear(screen.getByRole("textbox"));
  await usuario.type(screen.getByRole("textbox"), "Corrigido");
  await usuario.click(screen.getByRole("button", { name: "Salvar comentário" }));
  expect(screen.getByRole("button", { name: /— Corrigido/ })).toBeInTheDocument();
  await usuario.click(screen.getByRole("button", { name: /Remover comentário/ }));
  expect(screen.getByRole("button", { name: "Enviar comentários ao agente (0)" })).toBeDisabled();
});

it("mostra falha de patch obsoleto e permite atualizar", async () => {
  vi.mocked(service.aplicarRevisao).mockRejectedValueOnce(new Error("O arquivo mudou. Atualize a revisão antes de aplicar."));
  montar();
  await usuario.click(await screen.findByRole("button", { name: "Stage do trecho" }));
  expect(await screen.findByRole("alert")).toHaveTextContent("O arquivo mudou");
  await usuario.click(screen.getByRole("button", { name: "Atualizar" }));
  await waitFor(() => expect(screen.queryByRole("alert")).not.toBeInTheDocument());
});

it("mostra binário sem ações granulares e árvore limpa ao atualizar", async () => {
  vi.mocked(service.listarRevisao).mockResolvedValueOnce([{ ...arquivo, granular: false, trechos: [] }]).mockResolvedValue([]);
  montar();
  expect(await screen.findByText(/Arquivo binário, vazio/)).toBeInTheDocument();
  expect(screen.queryByRole("button", { name: "Stage do trecho" })).not.toBeInTheDocument();
  await usuario.click(screen.getByRole("button", { name: "Atualizar" }));
  expect(await screen.findByText("Nenhuma alteração neste escopo.")).toBeInTheDocument();
});

it("envia pelo atalho do rodapé e bloqueia envio sem sessão", async () => {
  const view = montar();
  await comentar(/Comentar linha 1 atual/, "Revisar");
  fireEvent.keyDown(screen.getByRole("button", { name: "Enviar comentários ao agente (1)" }), { key: "Enter", metaKey: true });
  await waitFor(() => expect(service.enviarComentarios).toHaveBeenCalledTimes(1));
  view.unmount();
  render(<RevisaoPane projectPath="/projeto" />);
  await screen.findByRole("button", { name: "Stage do trecho" });
  expect(screen.getByRole("button", { name: "Enviar comentários ao agente (0)" })).toBeDisabled();
});
