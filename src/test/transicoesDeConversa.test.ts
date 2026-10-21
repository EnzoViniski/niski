// Fork: transições nunca podem atrasar para sempre, duplicar ou reordenar ações de navegação.
import { criarDespachoComTransicao, nomeDaTransicao } from "../features/workspace/transicoesDeConversa";
import { setMotionLevel } from "../lib/motion";
import type { WorkspacesAction } from "../types/workspace";

function prometer() { let terminar!: () => void; const promise = new Promise<void>((resolve) => { terminar = resolve; }); return { promise, terminar }; }
function painel(id: string, conversa: string) {
  const elemento = document.createElement("section"); elemento.dataset.dropPane = id; elemento.dataset.transicaoPainel = conversa;
  document.body.appendChild(elemento); return elemento;
}
function instalar() {
  const transicoes: { atualizar: () => void; terminar: () => void; skipTransition: ReturnType<typeof vi.fn> }[] = [];
  const iniciar = vi.fn((atualizar: () => void) => {
    const fim = prometer(); const skipTransition = vi.fn();
    transicoes.push({ atualizar, terminar: fim.terminar, skipTransition });
    return { ready: Promise.resolve(), finished: fim.promise, skipTransition };
  });
  Object.defineProperty(document, "startViewTransition", { value: iniciar, configurable: true });
  return { transicoes, iniciar };
}

beforeEach(() => { document.body.innerHTML = ""; setMotionLevel("completas"); });
afterEach(() => {
  Reflect.deleteProperty(document, "startViewTransition");
  document.querySelectorAll("style[data-transicao-conversa]").forEach((el) => el.remove());
  setMotionLevel("completas");
});
const escolher = (tabId: string): WorkspacesAction => ({ type: "SELECT_TAB", paneId: "painel", tabId });

it("sem API nativa ou com animações desligadas navega imediatamente", () => {
  const aplicar = vi.fn();
  criarDespachoComTransicao(aplicar)(escolher("a"));
  expect(aplicar).toHaveBeenCalledOnce();
  const { iniciar } = instalar(); setMotionLevel("desligadas");
  criarDespachoComTransicao(aplicar)(escolher("b"));
  expect(iniciar).not.toHaveBeenCalled(); expect(aplicar).toHaveBeenCalledTimes(2);
});

it("preserva a ordem das ações recebidas enquanto o WebKit captura a tela", () => {
  const { transicoes } = instalar(); const aplicar = vi.fn(); const dispatch = criarDespachoComTransicao(aplicar);
  const trocar = escolher("nova"); const sincronizar: WorkspacesAction = { type: "SYNC_SESSION_IDENTITIES", sessions: [] };
  dispatch(trocar); dispatch(sincronizar);
  expect(aplicar).not.toHaveBeenCalled();
  transicoes[0].atualizar(); transicoes[0].atualizar();
  expect(aplicar.mock.calls.map(([acao]) => acao)).toEqual([trocar, sincronizar]);
});

it("só anima o painel alterado e limpa o estilo quando a animação acaba", async () => {
  const { transicoes } = instalar(); const alvo = painel("painel", "antiga"); painel("vizinho", "sem-mudanca");
  criarDespachoComTransicao(() => { alvo.dataset.transicaoPainel = "nova"; })(escolher("nova"));
  transicoes[0].atualizar();
  const estilo = document.querySelector("style[data-transicao-conversa]");
  expect(estilo?.textContent).toContain(nomeDaTransicao("painel"));
  expect(estilo?.textContent).not.toContain(nomeDaTransicao("vizinho"));
  transicoes[0].terminar(); await Promise.resolve(); await Promise.resolve();
  expect(estilo?.isConnected).toBe(false);
});

it("cliques rápidos pulam a animação anterior sem perder a próxima nem apagar o estilo dela", async () => {
  const { transicoes } = instalar(); const aplicar = vi.fn(); const dispatch = criarDespachoComTransicao(aplicar);
  dispatch(escolher("a")); transicoes[0].atualizar();
  dispatch(escolher("b")); transicoes[1].atualizar();
  expect(transicoes[0].skipTransition).toHaveBeenCalledOnce();
  expect(aplicar.mock.calls.map(([acao]) => acao.tabId)).toEqual(["a", "b"]);
  transicoes[0].terminar(); await Promise.resolve(); await Promise.resolve();
  expect(document.querySelectorAll("style[data-transicao-conversa]")).toHaveLength(1);
  transicoes[1].terminar(); await Promise.resolve(); await Promise.resolve();
  expect(document.querySelectorAll("style[data-transicao-conversa]")).toHaveLength(0);
});

it("falha de captura aplica a navegação uma única vez, mesmo se o callback já rodou", () => {
  const aplicar = vi.fn();
  Object.defineProperty(document, "startViewTransition", { configurable: true, value: (atualizar: () => void) => {
    atualizar(); throw new Error("captura cancelada");
  } });
  criarDespachoComTransicao(aplicar)(escolher("a"));
  expect(aplicar).toHaveBeenCalledOnce();
  expect(document.querySelector("style[data-transicao-conversa]")).toBeNull();
});

it("fechar um painel anima apenas o snapshot de saída, sem manter conteúdo vivo", () => {
  const { transicoes } = instalar(); const alvo = painel("painel", "a");
  criarDespachoComTransicao(() => alvo.remove())({ type: "CLOSE_PANE", paneId: "painel" });
  transicoes[0].atualizar();
  expect(alvo.isConnected).toBe(false);
  expect(document.querySelector("style[data-transicao-conversa]")?.textContent).toContain(`::view-transition-old(${nomeDaTransicao("painel")})`);
});
