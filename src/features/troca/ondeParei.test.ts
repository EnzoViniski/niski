// Fork (troca-de-contexto): o cartão "onde parei" montado de timelines falsas das três CLIs.
import { describe, expect, it } from "vitest";
import type { Conversation, Mensagem } from "../../mobile/api";
import {
  AUSENCIA_CURTA_MS, AUSENCIA_LONGA_MS, contarFeitos, deveMostrar, emUmaLinha, estadoAgora, linhaEssencial,
  montarOndeParei, pedacosDosFeitos, ultimoIndice,
} from "./ondeParei";

const AGORA = 1_800_000_000_000;

function msg(n: number, campos: Partial<Mensagem> & { provider: string }): Mensagem {
  return { id: `c1:${n}`, role: "assistant", text: "", kind: "text", ...campos };
}

function conversa(campos: Partial<Conversation> = {}): Conversation {
  return {
    id: "c1", title: "Conversa", project_id: "p", provider: "claude", profile_id: null, state: "answered", live: true,
    capabilities: { prompt: true, approve: false, revision: "r", approval_text: null, reason: null },
    ...campos,
  };
}

/** Claude: pedido, edição com diff, testes que falharam, correção, testes que passaram, resposta. */
const claude: Mensagem[] = [
  msg(0, { provider: "claude", role: "user", text: "Arruma o login\nque está quebrando no Safari" }),
  msg(1, { provider: "claude", kind: "tool", tool: "Read", text: "/repo/src/login.ts" }),
  msg(2, { provider: "claude", kind: "tool", tool: "Edit", text: "/repo/src/login.ts",
    diffs: [{ arquivo: "/repo/src/login.ts", tipo: "editado", adicionadas: 12, removidas: 3, linhas: [] }] }),
  msg(3, { provider: "claude", kind: "tool", tool: "Bash", text: "npm test", ok: false }),
  msg(4, { provider: "claude", kind: "tool", tool: "Edit", text: "/repo/src/login.ts",
    diffs: [{ arquivo: "/repo/src/login.ts", tipo: "editado", adicionadas: 2, removidas: 1, linhas: [] }] }),
  msg(5, { provider: "claude", kind: "tool", tool: "Write", text: "/repo/src/login.test.ts",
    diffs: [{ arquivo: "/repo/src/login.test.ts", tipo: "novo", adicionadas: 30, removidas: 0, linhas: [] }] }),
  msg(6, { provider: "claude", kind: "tool", tool: "Bash", text: "npm test -- login", ok: true }),
  msg(7, { provider: "claude", kind: "tool", tool: "TodoWrite", text: "3 tarefas" }),
  msg(8, { provider: "claude", text: "## Pronto\n\nO login voltou a funcionar no Safari. **Os testes passaram.**" }),
];

/** Codex: Shell com `ok`, patch com diff, plano. */
const codex: Mensagem[] = [
  msg(0, { provider: "codex", role: "user", text: "Rode o cargo test e corrija o que falhar" }),
  msg(1, { provider: "codex", kind: "tool", tool: "Shell", text: "cargo test", ok: false }),
  msg(2, { provider: "codex", kind: "tool", tool: "apply_patch", text: "crates/a/src/lib.rs",
    diffs: [{ arquivo: "crates/a/src/lib.rs", tipo: "editado", adicionadas: 4, removidas: 4, linhas: [] }] }),
  msg(3, { provider: "codex", kind: "tool", tool: "Shell", text: "cargo test", ok: false }),
  msg(4, { provider: "codex", text: "Ainda falta um teste; vou olhar o parser." }),
];

/** Antigravity: só falas e ações com resumo (sem diff, sem `ok`). */
const antigravity: Mensagem[] = [
  msg(0, { provider: "antigravity", role: "user", text: "Crie a página de preços" }),
  msg(1, { provider: "antigravity", kind: "tool", tool: "write_to_file", text: "Criou a página de preços com três planos" }),
  msg(2, { provider: "antigravity", kind: "tool", tool: "run_command", text: "npm run build" }),
  msg(3, { provider: "antigravity", text: "Fiz a página. Quer que eu ajuste as cores?" }),
];

describe("emUmaLinha", () => {
  it("tira markdown, junta linhas e corta", () => {
    expect(emUmaLinha("## Título\n\n- um\n- dois", 80)).toBe("Título · um · dois");
    expect(emUmaLinha("a".repeat(50), 10)).toBe(`${"a".repeat(9)}…`);
    expect(emUmaLinha("", 10)).toBeNull();
    expect(emUmaLinha("---\n\n", 10)).toBeNull();
  });
});

describe("contarFeitos", () => {
  it("Claude: arquivos somados, testes pelo último comando de teste", () => {
    const feitos = contarFeitos(claude);
    expect(feitos.arquivos.map((a) => a.nome)).toEqual(["login.ts", "login.test.ts"]);
    expect(feitos.adicionadas).toBe(44);
    expect(feitos.removidas).toBe(4);
    expect(feitos.comandos).toBe(2);
    expect(feitos.comandosQueFalharam).toBe(1);
    expect(feitos.testes).toEqual({ comando: "npm test -- login", ok: true });
    expect(feitos.leituras).toBe(1);
    expect(feitos.ultimaAcao).toBe("Rodando: npm test -- login"); // o TodoWrite não conta
  });

  it("Codex: testes falhando", () => {
    const feitos = contarFeitos(codex);
    expect(feitos.testes).toEqual({ comando: "cargo test", ok: false });
    expect(feitos.comandosQueFalharam).toBe(2);
    expect(pedacosDosFeitos(feitos)[0]).toEqual({ texto: "Testes falharam", tom: "falha" });
  });

  it("Antigravity: edição sem arquivo e comando sem veredito", () => {
    const feitos = contarFeitos(antigravity);
    expect(feitos.arquivos).toEqual([]);
    expect(feitos.edicoesSemArquivo).toBe(1);
    expect(feitos.comandos).toBe(1);
    expect(feitos.testes).toBeNull();
    expect(pedacosDosFeitos(feitos).map((p) => p.texto)).toEqual(["1 edição", "1 comando"]);
  });
});

describe("estadoAgora", () => {
  it("aprovação do gancho traz o que aprovar", () => {
    const estado = estadoAgora(conversa({ state: "approval_required", capabilities: {
      prompt: false, approve: true, revision: "r", approval_text: "Allow?", reason: null,
      pedido: { id: "p1", origem: "gancho", ferramenta: "Bash", resumo: "Bash · rm -rf dist", expira_em_ms: 0 },
    } }));
    expect(estado).toEqual({ tom: "precisa", rotulo: "Esperando sua aprovação", detalhe: "Bash · rm -rf dist" });
  });

  it("pergunta do agente, trabalhando, pronto, erro e sem sessão", () => {
    const pergunta = estadoAgora(conversa({ capabilities: { prompt: false, approve: false, revision: "r", approval_text: null, reason: null,
      escolha: { pergunta: "Qual banco usar?", opcoes: [], da_cli: false } as never } }));
    expect(pergunta.tom).toBe("precisa");
    expect(pergunta.detalhe).toBe("Qual banco usar?");
    expect(estadoAgora(conversa({ state: "working" }), "Rodando: npm test")).toMatchObject({ tom: "trabalhando", detalhe: "Rodando: npm test" });
    expect(estadoAgora(conversa({ state: "answered" })).tom).toBe("pronto");
    expect(estadoAgora(conversa({ state: "answered", notice: "usage_limit" })).tom).toBe("erro");
    expect(estadoAgora(null).rotulo).toBe("Sem estado ainda");
  });
});

describe("montarOndeParei", () => {
  it("sem olhada: resume o último pedido inteiro", () => {
    const cartao = montarOndeParei({ mensagens: claude, conversa: conversa(), olhada: null, agora: AGORA });
    expect(cartao.pedido).toBe("Arruma o login · que está quebrando no Safari");
    expect(cartao.janela).toBe("pedido");
    expect(cartao.novas).toBe(0);
    expect(cartao.ultimaFala).toBe("Pronto · O login voltou a funcionar no Safari. Os testes passaram.");
    expect(cartao.estado.tom).toBe("pronto");
    expect(cartao.ausenciaMs).toBeNull();
  });

  it("com olhada antiga: conta só o que chegou depois", () => {
    const olhada = { n: 3, em: AGORA - 5 * 60_000, tom: "trabalhando" as const };
    const cartao = montarOndeParei({ mensagens: claude, conversa: conversa(), olhada, agora: AGORA });
    expect(cartao.janela).toBe("olhada");
    expect(cartao.novas).toBe(5);
    expect(cartao.primeiroNovo).toBe(4);
    expect(cartao.feitos.comandos).toBe(1);
    expect(cartao.feitos.testes?.ok).toBe(true);
    expect(cartao.ausenciaMs).toBe(5 * 60_000);
    expect(deveMostrar(cartao, olhada)).toBe(true);
  });

  it("a fronteira congelada vence a olhada (a olhada já andou enquanto você lê)", () => {
    const olhada = { n: 8, em: AGORA, tom: "pronto" as const };
    const cartao = montarOndeParei({ mensagens: claude, conversa: conversa(), olhada, agora: AGORA, desde: 6 });
    expect(cartao.novas).toBe(2);
    expect(cartao.primeiroNovo).toBe(7);
  });

  it("página truncada: avisa que a contagem é um mínimo", () => {
    const pagina = claude.slice(5);
    const cartao = montarOndeParei({ mensagens: pagina, conversa: conversa(), olhada: { n: 1, em: AGORA - 60_000 }, agora: AGORA });
    expect(cartao.truncado).toBe(true);
    expect(cartao.pedido).toBeNull();
  });

  it("plano do agente e resumo estruturado (ponto de extensão do harness)", () => {
    const cartao = montarOndeParei({
      mensagens: codex, olhada: null, agora: AGORA,
      tarefas: [{ texto: "a", estado: "feita" }, { texto: "b", estado: "andamento", fazendo: "Corrigindo o parser" }, { texto: "c", estado: "pendente" }],
      conversa: { ...conversa({ provider: "codex", state: "working" }), resumo_do_agente: { proximo: "Rodar os testes de novo", lixo: 3 } } as Conversation,
    });
    expect(cartao.plano).toEqual({ feitas: 1, total: 3, fazendo: "Corrigindo o parser" });
    expect(cartao.extra).toEqual({ fazendo: undefined, proximo: "Rodar os testes de novo", bloqueio: undefined });
    expect(cartao.provider).toBe("codex");
  });

  it("Antigravity degrada sem diff nem veredito", () => {
    const cartao = montarOndeParei({ mensagens: antigravity, conversa: conversa({ provider: "antigravity" }), olhada: null, agora: AGORA });
    expect(cartao.feitos.testes).toBeNull();
    expect(cartao.ultimaFala).toBe("Fiz a página. Quer que eu ajuste as cores?");
    expect(linhaEssencial(cartao.estado, cartao)).toBe("Terminou: Fiz a página. Quer que eu ajuste as cores?");
  });
});

describe("deveMostrar", () => {
  const base = montarOndeParei({ mensagens: claude, conversa: conversa(), olhada: null, agora: AGORA });
  it("conversa vazia nunca mostra", () => {
    expect(deveMostrar(montarOndeParei({ mensagens: [], conversa: null, olhada: null, agora: AGORA }), null)).toBe(false);
  });
  it("volta rápida sem novidade não mostra; ausência longa mostra", () => {
    const olhada = { n: 8, em: AGORA - 10_000, tom: "pronto" as const };
    expect(deveMostrar({ ...base, novas: 0, ausenciaMs: 10_000 }, olhada)).toBe(false);
    expect(deveMostrar({ ...base, novas: 0, ausenciaMs: AUSENCIA_LONGA_MS }, olhada)).toBe(true);
  });
  it("novidade ou estado novo depois de meio minuto mostra", () => {
    const olhada = { n: 8, em: AGORA, tom: "trabalhando" as const };
    expect(deveMostrar({ ...base, novas: 0, ausenciaMs: AUSENCIA_CURTA_MS }, olhada)).toBe(true);
    expect(deveMostrar({ ...base, novas: 2, ausenciaMs: AUSENCIA_CURTA_MS - 1 }, { ...olhada, tom: "pronto" })).toBe(false);
  });
});

describe("linhaEssencial", () => {
  it("diz o que precisa de você", () => {
    expect(linhaEssencial({ tom: "precisa", rotulo: "Esperando sua aprovação", detalhe: "Bash · npm i" }, null)).toBe("Aprovar: Bash · npm i");
    expect(linhaEssencial({ tom: "precisa", rotulo: "Perguntou algo para você", detalhe: "Qual banco?" }, null)).toBe("Pergunta: Qual banco?");
    expect(linhaEssencial({ tom: "trabalhando", rotulo: "Trabalhando", detalhe: null }, null)).toBe("Trabalhando");
  });
  it("pronto com testes falhando avisa", () => {
    const cartao = montarOndeParei({ mensagens: codex, conversa: conversa({ provider: "codex" }), olhada: null, agora: AGORA });
    expect(linhaEssencial(cartao.estado, cartao)).toBe("Terminou (testes falharam): Ainda falta um teste; vou olhar o parser.");
  });
});

it("ultimoIndice", () => {
  expect(ultimoIndice(claude)).toBe(8);
  expect(ultimoIndice([])).toBe(-1);
});
