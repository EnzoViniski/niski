// Fork (troca-de-contexto): o cartão na tela — quando abre, some ao interagir, reabre, a marca de
// novo, a olhada gravada ao sair, o atalho e a linha da lateral.
import { useRef, useState } from "react";
import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Conversation, Mensagem } from "../../mobile/api";
import type { TerminalSession } from "../terminal/terminalService";
import type { AttentionItem } from "../terminal/useAttention";
import { CartaoOndeParei, MarcaDeNovo, haQuanto } from "./CartaoOndeParei";
import { EssencialNaTela, NovasNaTela, essencialDaSessao } from "./EssencialNaTela";
import { limparPainelDaTroca, pedirOndeParei, resumoNaTela } from "./painelDaTroca";
import { esquecerRoda, irParaOProximo, proximoDaRoda } from "./proximoAgente";
import { lerOlhada, marcarOlhada } from "./ultimaOlhada";
import { OlhandoContext, useOndeParei } from "./useOndeParei";

const conversa: Conversation = {
  id: "c1", title: "Login", project_id: "p", provider: "claude", profile_id: null, state: "answered", live: true,
  capabilities: { prompt: true, approve: false, revision: "r", approval_text: null, reason: null },
};

const mensagens: Mensagem[] = [
  { id: "c1:0", role: "user", kind: "text", provider: "claude", text: "Arruma o login" },
  { id: "c1:1", role: "assistant", kind: "tool", tool: "Bash", provider: "claude", text: "npm test", ok: false },
  { id: "c1:2", role: "assistant", kind: "tool", tool: "Edit", provider: "claude", text: "/r/login.ts",
    diffs: [{ arquivo: "/r/login.ts", tipo: "editado", adicionadas: 3, removidas: 1, linhas: [] }] },
  { id: "c1:3", role: "assistant", kind: "tool", tool: "Bash", provider: "claude", text: "npm test", ok: true },
  { id: "c1:4", role: "assistant", kind: "text", provider: "claude", text: "Pronto, os testes passaram." },
];

function Tela({ msgs = mensagens, conv = conversa }: { msgs?: Mensagem[]; conv?: Conversation }) {
  const tela = useRef<HTMLElement>(null);
  const onde = useOndeParei({ conversa: conv, mensagens: msgs, carregado: true, tela });
  return (
    <section ref={tela}>
      <CartaoOndeParei onde={onde} tela={tela} />
      {onde.marca !== null && <MarcaDeNovo />}
      <textarea aria-label="Mensagem" />
    </section>
  );
}

function ComFoco({ inicial, ...props }: { inicial: boolean; msgs?: Mensagem[] }) {
  const [foco, setFoco] = useState(inicial);
  return (
    <>
      <button type="button" onClick={() => setFoco((atual) => !atual)}>alternar foco</button>
      <OlhandoContext.Provider value={foco}><Tela {...props} /></OlhandoContext.Provider>
    </>
  );
}

beforeEach(() => {
  localStorage.clear(); limparPainelDaTroca(); esquecerRoda();
  vi.spyOn(document, "hasFocus").mockReturnValue(true); // o jsdom diz que a janela nunca tem foco
});
afterEach(() => { localStorage.clear(); vi.restoreAllMocks(); });

describe("cartão 'onde parei' na conversa", () => {
  it("voltou depois de um tempo com novidade: abre com o que chegou, a marca e o resumo", () => {
    marcarOlhada("c1", { n: 1, em: Date.now() - 12 * 60_000, tom: "trabalhando" });
    render(<Tela />);
    const cartao = screen.getByRole("region", { name: "Onde parei" });
    expect(cartao).toHaveTextContent("Terminou, esperando sua próxima mensagem");
    expect(cartao).toHaveTextContent("3 novidades · você saiu há 12 min");
    expect(cartao).toHaveTextContent("Você pediuArruma o login");
    expect(cartao).toHaveTextContent("Testes passaram");
    expect(cartao).toHaveTextContent("Editou login.ts (+3 −1)");
    expect(screen.getByRole("separator", { name: "Novo desde a sua última olhada" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Ver o que chegou" })).toBeInTheDocument();
    // Olhando, a olhada anda junto com a timeline.
    expect(lerOlhada("c1")?.n).toBe(4);
  });

  it("some ao digitar, reabre pelo botão e fecha com Esc", () => {
    marcarOlhada("c1", { n: 1, em: Date.now() - 60 * 60_000 });
    render(<Tela />);
    fireEvent.keyDown(screen.getByLabelText("Mensagem"), { key: "a" });
    expect(screen.queryByRole("region", { name: "Onde parei" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Onde parei" }));
    expect(screen.getByRole("region", { name: "Onde parei" })).toBeInTheDocument();
    fireEvent.keyDown(screen.getByLabelText("Mensagem"), { key: "Escape" });
    expect(screen.queryByRole("region", { name: "Onde parei" })).toBeNull();
  });

  it("volta rápida sem novidade não abre", () => {
    marcarOlhada("c1", { n: 4, em: Date.now() - 5_000, tom: "pronto" });
    render(<Tela />);
    expect(screen.queryByRole("region", { name: "Onde parei" })).toBeNull();
    expect(screen.queryByRole("separator")).toBeNull();
  });

  it("painel sem foco: não abre, não marca como visto e a lateral recebe as novidades; ao focar, abre", () => {
    marcarOlhada("c1", { n: 1, em: Date.now() - 10 * 60_000, tom: "trabalhando" });
    render(<ComFoco inicial={false} />);
    expect(screen.queryByRole("region", { name: "Onde parei" })).toBeNull();
    expect(lerOlhada("c1")?.n).toBe(1);
    expect(resumoNaTela("c1")).toEqual({ tom: "pronto", linha: "Terminou: Pronto, os testes passaram.", novas: 3 });
    fireEvent.click(screen.getByRole("button", { name: "alternar foco" }));
    expect(screen.getByRole("region", { name: "Onde parei" })).toBeInTheDocument();
    expect(resumoNaTela("c1")?.novas).toBe(0);
  });

  it("ao sair do painel grava a olhada com a hora", () => {
    render(<ComFoco inicial />);
    const antes = Date.now();
    fireEvent.click(screen.getByRole("button", { name: "alternar foco" }));
    const olhada = lerOlhada("c1");
    expect(olhada?.n).toBe(4);
    expect(olhada?.em).toBeGreaterThanOrEqual(antes);
    expect(olhada?.tom).toBe("pronto");
  });

  it("janela sem foco (outro app na frente) não conta como olhar", () => {
    vi.spyOn(document, "hasFocus").mockReturnValue(false);
    marcarOlhada("c1", { n: 1, em: Date.now() - 10 * 60_000 });
    render(<Tela />);
    expect(screen.queryByRole("region", { name: "Onde parei" })).toBeNull();
    expect(lerOlhada("c1")?.n).toBe(1);
  });

  it("o atalho abre o cartão mesmo sem novidade", () => {
    marcarOlhada("c1", { n: 4, em: Date.now() - 1_000, tom: "pronto" });
    render(<Tela />);
    expect(screen.queryByRole("region", { name: "Onde parei" })).toBeNull();
    act(() => pedirOndeParei("c1"));
    expect(screen.getByRole("region", { name: "Onde parei" })).toBeInTheDocument();
  });

  it("pedido feito antes de o painel ganhar foco abre quando ele ganha", () => {
    marcarOlhada("c1", { n: 4, em: Date.now() - 1_000, tom: "pronto" });
    render(<ComFoco inicial={false} />);
    act(() => pedirOndeParei("c1"));
    expect(screen.queryByRole("region", { name: "Onde parei" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "alternar foco" }));
    expect(screen.getByRole("region", { name: "Onde parei" })).toBeInTheDocument();
  });

  it("esperando aprovação: mostra o que aprovar e o botão Responder", () => {
    const pedindo: Conversation = { ...conversa, state: "approval_required", capabilities: {
      prompt: false, approve: true, revision: "r", approval_text: "Allow?", reason: null,
      pedido: { id: "x", origem: "gancho", ferramenta: "Bash", resumo: "Bash · rm -rf dist", expira_em_ms: 0 },
    } };
    render(<Tela conv={pedindo} />); // nunca olhou nesta versão: resumo para começar
    const cartao = screen.getByRole("region", { name: "Onde parei" });
    expect(cartao).toHaveTextContent("Esperando sua aprovação");
    expect(cartao).toHaveTextContent("Bash · rm -rf dist");
    expect(screen.getByRole("button", { name: "Responder" })).toBeInTheDocument();
  });
});

describe("lateral e atalho", () => {
  const sessao = { id: "s1", conversation_id: "c9", state: "approval_required", provider: "codex" } as unknown as TerminalSession;

  it("sem conversa montada, a linha cai no estado da sessão", () => {
    render(<><EssencialNaTela sessao={sessao} id="e1" /><NovasNaTela sessao={sessao} /></>);
    expect(document.getElementById("e1")).toHaveTextContent("Esperando sua aprovação");
    expect(essencialDaSessao({ state: "working" } as TerminalSession)).toEqual({ tom: "trabalhando", linha: "Trabalhando" });
  });

  it("a roda passa por todos os que precisam de você e pede o cartão", () => {
    const lista = [{ conversationKey: "a" }, { conversationKey: "b" }, { conversationKey: "c" }];
    expect(proximoDaRoda(lista, null)?.conversationKey).toBe("a");
    expect(proximoDaRoda(lista, "a")?.conversationKey).toBe("b");
    expect(proximoDaRoda(lista, "c")?.conversationKey).toBe("a");
    expect(proximoDaRoda(lista, "sumiu")?.conversationKey).toBe("a");
    expect(proximoDaRoda([], "a")).toBeNull();
    const itens = lista as AttentionItem[];
    expect(irParaOProximo(itens)?.conversationKey).toBe("a");
    expect(irParaOProximo(itens)?.conversationKey).toBe("b");
  });

  it("haQuanto", () => {
    expect(haQuanto(10_000)).toBe("agora");
    expect(haQuanto(5 * 60_000)).toBe("há 5 min");
    expect(haQuanto(3 * 3_600_000)).toBe("há 3 h");
    expect(haQuanto(26 * 3_600_000)).toBe("há 1 dia");
  });
});
