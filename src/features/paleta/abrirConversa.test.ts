import { describe, expect, it } from "vitest";
import { acoesParaAbrirConversa } from "./abrirConversa";
import { workspaceReducer } from "../workspace/workspaceReducer";
import { loadWorkspace, saveWorkspace, emptyWorkspace } from "../../services/workspaceService";
import type { Conversation } from "../../mobile/api";
import type { TerminalSession } from "../terminal/terminalService";
const conversa: Conversation = { id: "conv-1", project_id: "outro", title: "Conversa sintética", provider: "claude", profile_id: null, capabilities: null, state: null };
function workspace() { return workspaceReducer(emptyWorkspace(), { type: "ADD_PROJECT", path: "/sintetico" }); }

describe("abrir conversa pela paleta", () => {
  it("foca a sessão do projeto/workspace correto como a lateral", () => {
    const w = workspace(), projeto = w.projects[0];
    const sessao = { id: "s-1", conversation_id: conversa.id, project_id: projeto.id, state: "working" } as TerminalSession;
    expect(acoesParaAbrirConversa(conversa, [w], [sessao], null)).toEqual([
      { type: "FOCUS_SESSION", workspaceId: w.id, projectId: projeto.id, sessionId: "s-1", title: conversa.title },
    ]);
  });
  it("arquivo histórico abre sem iniciar CLI e reabrir foca a mesma aba", () => {
    let w = workspace();
    const p = w.projects[0];
    const acoes = acoesParaAbrirConversa(conversa, [w], [], p);
    expect(acoes).toEqual([{ type: "CREATE_TAB", kind: "conversa-historica", paneId: p.activePaneId, title: conversa.title, resourceId: conversa.id }]);
    for (const acao of acoes) if (acao.type === "CREATE_TAB") w = workspaceReducer(w, acao);
    const reabrir = acoesParaAbrirConversa(conversa, [w], [], w.projects[0]);
    expect(reabrir.map((a) => a.type)).toEqual(["SELECT_WORKSPACE", "SELECT_PROJECT", "SELECT_TAB"]);
  });
  it("histórico de outro projeto seleciona seu dono antes de criar a aba", () => {
    const w = workspace(), projeto = w.projects[0];
    const acoes = acoesParaAbrirConversa({ ...conversa, project_id: projeto.id }, [w], [], null);
    expect(acoes.map((a) => a.type)).toEqual(["SELECT_WORKSPACE", "SELECT_PROJECT", "CREATE_TAB"]);
  });
  it("aba de histórico continua válida depois de salvar e recarregar o workspace", () => {
    let w = workspace();
    w = workspaceReducer(w, { type: "CREATE_TAB", kind: "conversa-historica", title: conversa.title, resourceId: conversa.id, paneId: w.projects[0].activePaneId });
    saveWorkspace(w);
    expect(loadWorkspace()).toEqual(w);
    localStorage.clear();
  });
  it("sem projeto deixa o App abrir a conversa avulsa", () => {
    expect(acoesParaAbrirConversa(conversa, [], [], null)).toEqual([]);
  });
});
