// Fork (pergunta-da-cli): menu da própria CLI e "esperando algo no terminal" na Conversa.
import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { CartaoDePergunta } from "../mobile/CartaoDePergunta";
import { CartaoDeEspera } from "../mobile/conversa/CartaoDeEspera";
import { MobileApp } from "../mobile/MobileApp";
import type { Conversation, Escolha } from "../mobile/api";

vi.mock("../web/TerminalEspelho", () => ({ default: ({ saida, teclas }: { saida: string; teclas: string }) =>
  <div data-testid="terminal" data-saida={saida} data-teclas={teclas} /> }));

beforeEach(() => {
  history.replaceState(null, "", "/");
  sessionStorage.clear();
  window.scrollTo = vi.fn() as unknown as typeof window.scrollTo;
});
afterEach(() => vi.unstubAllGlobals());

// Textos do diálogo de acesso total do Codex (`codex-rs/tui`, permission_popups.rs).
const MENU_DA_CLI: Escolha = { pergunta: "Enable full access?", detalhe: null, da_cli: true,
  opcoes: [{ numero: 1, rotulo: "Yes, continue anyway" }, { numero: 2, rotulo: "Cancel" }] };

describe("cartão da pergunta da CLI", () => {
  it("diz que é a CLI, responde pelo número e abre o terminal", () => {
    const escolher = vi.fn();
    const abrir = vi.fn();
    render(<CartaoDePergunta escolha={MENU_DA_CLI} provider="codex" busy={false} onEscolher={escolher} onAvancar={vi.fn()} onAbrirTerminal={abrir} />);
    expect(screen.getByText("A CLI está perguntando")).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Enable full access?" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: /Cancel/ }));
    fireEvent.click(screen.getByRole("button", { name: "Abrir no terminal" }));
    fireEvent.click(screen.getByRole("button", { name: "Fechar" }));
    expect(escolher.mock.calls).toEqual([[2], [null]]);
    expect(abrir).toHaveBeenCalledOnce();
  });

  it("pergunta do agente continua como antes, sem o botão do terminal", () => {
    render(<CartaoDePergunta escolha={{ ...MENU_DA_CLI, da_cli: false, pergunta: "Qual banco usar?" }} provider="claude" busy={false}
      onEscolher={vi.fn()} onAvancar={vi.fn()} onAbrirTerminal={vi.fn()} />);
    expect(screen.getByText("O agente quer saber")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Abrir no terminal" })).toBeNull();
  });
});

describe("cartão de espera no terminal", () => {
  it("mostra as últimas linhas, a dica de teclas e abre o terminal", () => {
    const abrir = vi.fn();
    render(<CartaoDeEspera provider="claude" onAbrirTerminal={abrir}
      espera={{ texto: "Usage limit reached · limit resets 3:45pm\nContinuing automatically at 3:45pm · esc to cancel", dica: "Continuing automatically at 3:45pm · esc to cancel" }} />);
    expect(screen.getByRole("heading", { name: "O agente está esperando algo no terminal" })).toBeInTheDocument();
    expect(screen.getByLabelText("Últimas linhas do terminal")).toHaveTextContent("Usage limit reached · limit resets 3:45pm");
    expect(screen.getByText(/Teclas: Continuing automatically/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Abrir no terminal" }));
    expect(abrir).toHaveBeenCalledOnce();
  });
});

/** A conversa inteira: espera na tela vira cartão, o terminal abre por cima e o envio não destrava às cegas. */
async function abrirConversa(capabilities: Conversation["capabilities"]) {
  const PROJETOS = { published_at_ms: Date.now(), projects: [{ id: "p", name: "Projeto", path: "C:/p" }], agents: [], profiles: [], theme: null };
  const conversa: Conversation = { id: "c", title: "Chat", project_id: "p", provider: "claude", profile_id: "p", state: "answered", live: true, capabilities };
  const timeline = { timeline: { messages: [{ id: "c:0", role: "user", text: "oi", provider: "claude", kind: "text" }],
    next_cursor: null, prev_cursor: null, unavailable_segments: [] }, actions: [], conversation: conversa };
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
  return posts;
}

it("na conversa, a espera vira cartão e abre o terminal da conversa", async () => {
  await abrirConversa({ prompt: false, approve: false, revision: '"r"', approval_text: null, reason: null,
    espera: { texto: "Your usage limit has reset\nPress enter to confirm or esc to cancel", dica: "Press enter to confirm or esc to cancel" } });
  const cartao = await screen.findByRole("group", { name: "O agente está esperando algo no terminal" });
  fireEvent.click(within(cartao).getByRole("button", { name: "Abrir no terminal" }));
  const painel = await screen.findByRole("dialog", { name: "Responder no terminal" });
  expect(await within(painel).findByTestId("terminal")).toHaveAttribute("data-teclas", "/conversas/c/teclas");
});

it("na conversa, o menu da CLI responde pelo /escolher", async () => {
  const posts = await abrirConversa({ prompt: false, approve: false, revision: '"r"', approval_text: null, reason: null, escolha: MENU_DA_CLI });
  const cartao = await screen.findByRole("group", { name: "Enable full access?" });
  expect(within(cartao).getByText("A CLI está perguntando")).toBeInTheDocument();
  fireEvent.click(within(cartao).getByRole("button", { name: /Yes, continue anyway/ }));
  await waitFor(() => expect(posts.some((p) => p.url.endsWith("/escolher") && p.corpo === JSON.stringify({ opcao: 1 }))).toBe(true));
});
