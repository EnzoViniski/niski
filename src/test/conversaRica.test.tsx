// Fork: conversa rica — mini-diff, lista de tarefas, cartão do plano e selo do modo.
import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { BlocoDeAcoes } from "../mobile/Chat";
import { CartaoDoPlano } from "../mobile/CartaoDoPlano";
import { ListaDeTarefas } from "../mobile/ListaDeTarefas";
import { LINHAS_INICIAIS, MiniDiff, trechoAlterado } from "../mobile/MiniDiff";
import { agruparMensagens, MobileApp } from "../mobile/MobileApp";
import { modoVigente, SeloDoModo } from "../mobile/SeloDoModo";
import type { Conversation, DiffDeArquivo, Mensagem, Tarefa } from "../mobile/api";

beforeEach(() => {
  history.replaceState(null, "", "/");
  sessionStorage.clear();
  window.scrollTo = vi.fn() as unknown as typeof window.scrollTo;
});
afterEach(() => vi.unstubAllGlobals());

const diff = (extra: Partial<DiffDeArquivo> = {}): DiffDeArquivo => ({
  arquivo: "src/a.ts", tipo: "editado", adicionadas: 1, removidas: 1, linhas: [" import x", "-const a = 1;", "+const a = 2;"], ...extra,
});

const TAREFAS: Tarefa[] = [
  { texto: "Ler o código", estado: "feita" },
  { texto: "Rodar testes", estado: "andamento", fazendo: "Rodando testes" },
  { texto: "Abrir PR", estado: "pendente" },
];

describe("mini-diff", () => {
  it("pinta o que entra e o que sai e realça o trecho alterado da linha", () => {
    const { container } = render(<MiniDiff diffs={[diff()]} />);
    expect(container.querySelectorAll(".diff-linha-mais")).toHaveLength(1);
    expect(container.querySelectorAll(".diff-linha-menos")).toHaveLength(1);
    expect(container.querySelectorAll(".diff-linha-igual")).toHaveLength(1);
    expect([...container.querySelectorAll("mark.diff-realce")].map((m) => m.textContent)).toEqual(["1", "2"]);
    expect(trechoAlterado("abc", "xyz")).toBeNull();
  });

  it("mostra as primeiras linhas, 'ver mais' abre o resto e avisa do que o engine cortou", async () => {
    const linhas = Array.from({ length: 30 }, (_, i) => `+linha ${i}`);
    const { container } = render(<MiniDiff diffs={[diff({ tipo: "escrito", adicionadas: 90, removidas: 0, linhas, omitidas: 60 })]} />);
    expect(container.querySelectorAll(".diff-linha")).toHaveLength(LINHAS_INICIAIS);
    expect(screen.queryByText(/mais 60 linhas/)).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: `Ver mais ${30 - LINHAS_INICIAIS} linhas` }));
    expect(container.querySelectorAll(".diff-linha")).toHaveLength(30);
    expect(screen.getByText(/mais 60 linhas/)).toBeInTheDocument();
    expect(screen.getByText("arquivo inteiro")).toBeInTheDocument();
  });

  it("vários arquivos: cada um com nome e contagem; trecho vira separador", () => {
    const { container } = render(<MiniDiff diffs={[diff(), diff({ arquivo: "b.ts", tipo: "novo", adicionadas: 2, removidas: 0, linhas: ["@fn y", "+a", "+b"] })]} />);
    expect(screen.getByText("b.ts")).toHaveClass("diff-nome");
    expect(screen.getByText("novo")).toBeInTheDocument();
    expect(container.querySelector(".diff-linha-trecho")?.textContent).toContain("fn y");
  });

  it("na linha da ação: contagem à vista e o diff abre num toque", async () => {
    const acoes: Mensagem[] = [
      { id: "c:1", role: "assistant", text: "/proj/src/a.ts", provider: "claude", kind: "tool", tool: "Edit", diffs: [diff()] },
      { id: "c:2", role: "assistant", text: "1/3 · Rodando testes", provider: "claude", kind: "tool", tool: "TodoWrite", tarefas: TAREFAS },
    ];
    const { container } = render(<BlocoDeAcoes aoVivo acoes={acoes} />);
    // Cabeçalho do bloco soma as edições.
    expect(within(container.querySelector(".acoes-cabecalho")!).getByLabelText("1 linhas adicionadas, 1 removidas")).toBeInTheDocument();
    expect(screen.getByText("Tarefas 1/3")).toBeInTheDocument();
    expect(container.querySelector(".mini-diff")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: /Editou/ }));
    expect(container.querySelector(".mini-diff")).not.toBeNull();
  });
});

describe("lista de tarefas", () => {
  it("mostra o progresso e a tarefa atual, e recolhe lembrando a escolha", async () => {
    const { container, unmount } = render(<ListaDeTarefas tarefas={TAREFAS} conversa="c" ativo={false} />);
    expect(screen.getByText("1/3")).toBeInTheDocument();
    expect(screen.getByText("Rodando testes")).toBeInTheDocument();
    expect(container.querySelector(".tarefa.is-feita")).not.toBeNull();
    fireEvent.click(screen.getByRole("button", { name: /Tarefas/ }));
    expect(screen.queryByRole("list")).not.toBeInTheDocument();
    // Recolhida, a atual continua à vista no topo.
    expect(container.querySelector(".lista-tarefas-atual")?.textContent).toBe("Rodando testes");
    unmount();
    render(<ListaDeTarefas tarefas={TAREFAS} conversa="c" ativo={false} />);
    expect(screen.getByRole("button", { name: /Tarefas/ })).toHaveAttribute("aria-expanded", "false");
  });

  it("toda feita some com o agente parado e fica com ele trabalhando", () => {
    const feitas = TAREFAS.map((t) => ({ ...t, estado: "feita" as const }));
    const { container, rerender } = render(<ListaDeTarefas tarefas={feitas} conversa="d" ativo={false} />);
    expect(container).toBeEmptyDOMElement();
    rerender(<ListaDeTarefas tarefas={feitas} conversa="d" ativo />);
    expect(screen.getByText("3/3")).toBeInTheDocument();
    rerender(<ListaDeTarefas tarefas={[]} conversa="d" ativo />);
    expect(container).toBeEmptyDOMElement();
  });
});

describe("plano e modo", () => {
  it("plano em markdown; esperando aprovação mostra o selo e a pergunta dentro", () => {
    const { rerender } = render(<CartaoDoPlano plano={"# Migrar o login\n\n1. Trocar a rota"} provider="claude" />);
    expect(screen.getByRole("heading", { name: "Migrar o login" })).toBeInTheDocument();
    expect(screen.queryByText("Esperando sua aprovação")).not.toBeInTheDocument();
    rerender(<CartaoDoPlano plano="# P" pergunta={<button type="button">Aprovar</button>} />);
    expect(screen.getByText("Esperando sua aprovação")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Aprovar" })).toBeInTheDocument();
  });

  it("plano longo nasce recolhido", async () => {
    const longo = Array.from({ length: 40 }, (_, i) => `- passo ${i}`).join("\n");
    const { container } = render(<CartaoDoPlano plano={longo} />);
    expect(container.querySelector(".plano-corpo.is-recolhido")).not.toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Ver plano inteiro" }));
    expect(container.querySelector(".plano-corpo.is-recolhido")).toBeNull();
  });

  it("ExitPlanMode com plano vira bloco próprio, separado das ações", () => {
    const blocos = agruparMensagens([
      { id: "c:0", role: "assistant", text: "a.ts", provider: "claude", kind: "tool", tool: "Read" },
      { id: "c:1", role: "assistant", text: "Plano", provider: "claude", kind: "tool", tool: "ExitPlanMode", plano: "# Plano" },
      { id: "c:2", role: "assistant", text: "b.ts", provider: "claude", kind: "tool", tool: "Read" },
    ]);
    expect(blocos.map((b) => b.tipo)).toEqual(["acoes", "plano", "acoes"]);
  });

  it("selo do modo: rótulo em pt-BR, tela vale mais que o transcript", () => {
    const { rerender } = render(<SeloDoModo modo="acceptEdits" />);
    expect(screen.getByLabelText("Modo de permissão: Aceitar edições")).toBeInTheDocument();
    rerender(<SeloDoModo modo="bypassPermissions" />);
    expect(screen.getByLabelText("Modo de permissão: Sem permissões")).toHaveClass("is-alerta");
    rerender(<SeloDoModo modo={null} />);
    expect(screen.queryByLabelText(/Modo de permissão/)).not.toBeInTheDocument();
    expect(modoVigente("plan", "default")).toBe("plan");
    expect(modoVigente(null, "auto")).toBe("auto");
  });
});

/** A conversa inteira: plano esperando aprovação, lista acima do campo e o selo do modo. */
it("na conversa, a aprovação do plano vai pelo menu da CLI, dentro do cartão do plano", async () => {
  const PROJETOS = { published_at_ms: Date.now(), projects: [{ id: "p", name: "Projeto", path: "C:/p" }], agents: [], profiles: [], theme: null };
  const menu = { pergunta: "Would you like to proceed?", detalhe: null, opcoes: [
    { numero: 1, rotulo: "Yes, and auto-accept edits" }, { numero: 2, rotulo: "Yes, manually approve edits" }, { numero: 3, rotulo: "No, keep planning" }] };
  const conversa: Conversation = { id: "c", title: "Chat", project_id: "p", provider: "claude", profile_id: "p", state: "approval_required",
    capabilities: { prompt: false, approve: false, revision: '"r"', approval_text: null, reason: null, escolha: menu } };
  const timeline = { timeline: { messages: [
    { id: "c:0", role: "user", text: "planeje", provider: "claude", kind: "text" },
    { id: "c:1", role: "assistant", text: "Migrar o login", provider: "claude", kind: "tool", tool: "ExitPlanMode", plano: "# Migrar o login\n\n1. Trocar a rota" },
  ], next_cursor: null, prev_cursor: null, unavailable_segments: [], tarefas: TAREFAS, modo_de_permissao: "plan" }, actions: [], conversation: conversa };
  const posts: { url: string; corpo: string }[] = [];
  vi.stubGlobal("fetch", vi.fn(async (url: string, init?: RequestInit) => {
    if (init?.method === "POST") { posts.push({ url, corpo: String(init.body) }); return new Response(JSON.stringify({ id: "a" })); }
    return new Response(JSON.stringify(url.includes("timeline") ? timeline
      : url === "/projetos" ? PROJETOS : url === "/atencao" ? [] : [conversa]));
  }));
  render(<MobileApp />);
  const projeto = await screen.findByRole("button", { name: /Abrir projeto/ });
  await act(async () => { await userEvent.click(projeto); });
  const abrir = await screen.findByRole("button", { name: /Abrir conversa/ });
  await act(async () => { await userEvent.click(abrir); });

  const cartao = await screen.findByRole("article", { name: "Plano do agente" });
  expect(within(cartao).getByRole("heading", { name: "Migrar o login" })).toBeInTheDocument();
  // A pergunta aparece uma vez só — dentro do plano, não solta embaixo dele.
  expect(screen.getAllByText("Would you like to proceed?")).toHaveLength(1);
  expect(within(cartao).getByText("Would you like to proceed?")).toBeInTheDocument();
  expect(screen.getByRole("region", { name: "Lista de tarefas do agente" })).toBeInTheDocument();
  expect(screen.getByLabelText("Modo de permissão: Plano")).toBeInTheDocument();
  fireEvent.click(within(cartao).getByRole("button", { name: /No, keep planning/ }));
  await waitFor(() => expect(posts.some((p) => p.url.endsWith("/escolher") && p.corpo === JSON.stringify({ opcao: 3 }))).toBe(true));
});
