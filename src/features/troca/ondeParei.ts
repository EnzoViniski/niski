/**
 * Fork (troca-de-contexto): "onde parei" — o que aconteceu numa conversa desde a sua última olhada,
 * montado só do que o engine já manda (timeline, estado, plano, diffs). Determinístico, sem IA, sem
 * custo: é uma função pura sobre os dados que a tela já tem, testada com timelines falsas das três CLIs.
 *
 * O que cada CLI oferece é diferente e o cartão degrada sozinho:
 * - Claude: falas, ações com diff (Edit/Write/MultiEdit), lista de tarefas, `ok` das ações.
 * - Codex: falas, Shell/patch com diff, plano (`update_plan`), `ok` dos comandos.
 * - Antigravity: falas e ações com resumo (sem diff, sem `ok`, sem plano).
 */
import type { Conversation, Mensagem, Tarefa } from "../../mobile/api";
import type { Olhada, TomDaOlhada } from "./ultimaOlhada";

export type Tom = TomDaOlhada;

/** Onde a conversa está agora, numa frase. */
export interface EstadoAgora {
  tom: Tom;
  /** "Esperando sua aprovação", "Trabalhando", "Terminou"… */
  rotulo: string;
  /** O que exatamente: o comando a aprovar, a pergunta, a ação em andamento. */
  detalhe: string | null;
}

export interface ArquivoMudado { caminho: string; nome: string; adicionadas: number; removidas: number }

export interface Feitos {
  arquivos: ArquivoMudado[];
  /** Edições sem arquivo conhecido (Antigravity manda só um resumo da ação). */
  edicoesSemArquivo: number;
  adicionadas: number;
  removidas: number;
  comandos: number;
  comandosQueFalharam: number;
  leituras: number;
  buscas: number;
  outras: number;
  /** O último comando de teste da janela e se passou (`null`: a CLI não disse). */
  testes: { comando: string; ok: boolean | null } | null;
  /** A última ação, em uma linha ("Rodando: npm test"). */
  ultimaAcao: string | null;
}

/**
 * Ponto de extensão (harness): resumo estruturado que o próprio agente publica pelos ganchos. Se o
 * engine mandar `resumo_do_agente` no `/resumo` da conversa, o cartão mostra; sem ele, nada muda.
 */
export interface ResumoDoAgente {
  fazendo?: string;
  proximo?: string;
  bloqueio?: string;
  atualizado_em_ms?: number;
}

export interface OndeParei {
  provider: string | null;
  estado: EstadoAgora;
  /** O seu último pedido, em uma linha. */
  pedido: string | null;
  /** De onde contam os feitos: desde a sua olhada (houve novidade) ou o último pedido inteiro. */
  janela: "olhada" | "pedido";
  /** Mensagens (falas e ações) depois da sua última olhada. */
  novas: number;
  /** Chegou mais do que a página da timeline mostra (a contagem é um mínimo). */
  truncado: boolean;
  /** Número da primeira mensagem nova (para a marca "novo desde a sua última olhada"). */
  primeiroNovo: number | null;
  feitos: Feitos;
  plano: { feitas: number; total: number; fazendo: string | null } | null;
  /** O que o agente disse por último na janela, em uma linha. */
  ultimaFala: string | null;
  /** Quanto tempo desde a sua última olhada (ms); `null` se nunca olhou. */
  ausenciaMs: number | null;
  extra: ResumoDoAgente | null;
}

/** Número da mensagem (`<conversa>:<n>`), como o servidor numera. */
export function indiceDe(mensagem: Pick<Mensagem, "id">): number {
  const n = Number(mensagem.id.slice(mensagem.id.lastIndexOf(":") + 1));
  return Number.isFinite(n) ? n : -1;
}

/* ── Texto curto ─────────────────────────────────────────────────────────────────────────────── */

/** Tira a marcação de markdown do começo da linha (títulos, listas, citação) e as crases. */
function limparLinha(linha: string): string {
  return linha.replace(/^\s*(#{1,6}\s+|[-*+]\s+|>\s*|\d+[.)]\s+)/, "").replace(/[`*_]{1,3}/g, "").replace(/\s+/g, " ").trim();
}

/** Texto em uma linha só: as linhas viram " · ", cortado em `max` com reticências. */
export function emUmaLinha(texto: string | null | undefined, max: number): string | null {
  if (!texto) return null;
  const linhas = texto.split(/\r?\n/).map(limparLinha).filter((linha) => linha && !/^[-=_*]{3,}$/.test(linha));
  if (linhas.length === 0) return null;
  let saida = "";
  for (const linha of linhas) {
    const proxima = saida ? `${saida} · ${linha}` : linha;
    if (proxima.length > max) { saida = saida || linha; break; }
    saida = proxima;
  }
  return saida.length > max ? `${saida.slice(0, max - 1).trimEnd()}…` : saida;
}

function nomeDoArquivo(caminho: string): string {
  return caminho.split(/[\\/]/).filter(Boolean).pop() ?? caminho;
}

/* ── Ações ───────────────────────────────────────────────────────────────────────────────────── */

type TipoDeAcao = "edicao" | "comando" | "leitura" | "busca" | "ignorar" | "outra";

const TIPOS: Record<string, TipoDeAcao> = Object.fromEntries([
  ...["edit", "multiedit", "write", "notebookedit", "apply_patch", "write_to_file", "replace_file_content",
    "multi_replace_file_content", "edit_file", "create_file", "str_replace_editor"].map((n) => [n, "edicao"]),
  ...["bash", "powershell", "shell", "run_command", "exec_command", "local_shell", "container.exec"].map((n) => [n, "comando"]),
  ...["read", "view_file", "read_file", "view_code_item", "view_file_outline"].map((n) => [n, "leitura"]),
  ...["grep", "glob", "websearch", "webfetch", "grep_search", "find_by_name", "codebase_search", "list_dir",
    "search_web", "read_url_content"].map((n) => [n, "busca"]),
  // Controle da própria conversa: a lista de tarefas e o plano já aparecem no cartão.
  ...["todowrite", "update_plan", "taskcreate", "taskupdate", "askuserquestion", "ask_question", "exitplanmode"].map((n) => [n, "ignorar"]),
]) as Record<string, TipoDeAcao>;

function tipoDaAcao(mensagem: Mensagem): TipoDeAcao {
  return TIPOS[(mensagem.tool ?? "").toLowerCase()] ?? "outra";
}

const VERBO: Record<TipoDeAcao, string> = {
  edicao: "Editando", comando: "Rodando", leitura: "Lendo", busca: "Procurando", ignorar: "", outra: "",
};

/** Comando de teste (npm test, vitest, cargo test, pytest…). */
const TESTE = /\b(?:(?:npm|pnpm|yarn|bun)\s+(?:run\s+)?test(?::\S+)?|vitest|jest|pytest|py\.test|cargo\s+(?:test|nextest)|go\s+test|mocha|playwright\s+test|dotnet\s+test|mvn\s+\S*\s*test|gradlew?\s+\S*test|rspec|phpunit|deno\s+test|swift\s+test|make\s+test|tox|ctest)\b/i;

export function ehComandoDeTeste(texto: string): boolean {
  return TESTE.test(texto);
}

function textoDaAcao(mensagem: Mensagem): string {
  const tipo = tipoDaAcao(mensagem);
  const texto = emUmaLinha(mensagem.text, 90) ?? "";
  if (tipo === "edicao" || tipo === "leitura") {
    const caminho = mensagem.diffs?.[0]?.arquivo ?? texto;
    return `${VERBO[tipo]}: ${nomeDoArquivo(caminho)}`;
  }
  if (VERBO[tipo]) return texto ? `${VERBO[tipo]}: ${texto}` : VERBO[tipo];
  return texto ? `${mensagem.tool ?? "Ação"}: ${texto}` : mensagem.tool ?? "Ação";
}

export function contarFeitos(mensagens: Mensagem[]): Feitos {
  const feitos: Feitos = {
    arquivos: [], edicoesSemArquivo: 0, adicionadas: 0, removidas: 0, comandos: 0, comandosQueFalharam: 0,
    leituras: 0, buscas: 0, outras: 0, testes: null, ultimaAcao: null,
  };
  const porCaminho = new Map<string, ArquivoMudado>();
  for (const mensagem of mensagens) {
    if (mensagem.kind !== "tool") continue;
    const tipo = tipoDaAcao(mensagem);
    if (tipo === "ignorar") continue;
    feitos.ultimaAcao = textoDaAcao(mensagem);
    if (tipo === "edicao") {
      if (mensagem.diffs?.length) {
        for (const diff of mensagem.diffs) {
          const atual = porCaminho.get(diff.arquivo) ?? { caminho: diff.arquivo, nome: nomeDoArquivo(diff.arquivo), adicionadas: 0, removidas: 0 };
          atual.adicionadas += diff.adicionadas;
          atual.removidas += diff.removidas;
          porCaminho.set(diff.arquivo, atual);
        }
      } else {
        // Sem diff: o texto da ação é o caminho (Claude/Codex) ou um resumo (Antigravity).
        const caminhos = (mensagem.text ?? "").split(/,\s*/).map((c) => c.replace(/…$/, "").trim()).filter((c) => /[\\/]|\.\w{1,6}$/.test(c) && !/\s/.test(c));
        if (caminhos.length === 0) feitos.edicoesSemArquivo += 1;
        for (const caminho of caminhos) if (!porCaminho.has(caminho)) porCaminho.set(caminho, { caminho, nome: nomeDoArquivo(caminho), adicionadas: 0, removidas: 0 });
      }
    } else if (tipo === "comando") {
      feitos.comandos += 1;
      if (mensagem.ok === false) feitos.comandosQueFalharam += 1;
      if (ehComandoDeTeste(mensagem.text ?? "")) feitos.testes = { comando: emUmaLinha(mensagem.text, 60) ?? "", ok: mensagem.ok ?? null };
    } else if (tipo === "leitura") feitos.leituras += 1;
    else if (tipo === "busca") feitos.buscas += 1;
    else feitos.outras += 1;
  }
  feitos.arquivos = [...porCaminho.values()];
  feitos.adicionadas = feitos.arquivos.reduce((soma, a) => soma + a.adicionadas, 0);
  feitos.removidas = feitos.arquivos.reduce((soma, a) => soma + a.removidas, 0);
  return feitos;
}

/* ── Estado ──────────────────────────────────────────────────────────────────────────────────── */

/** Onde a conversa está agora, pelo resumo vivo do engine (estado, capacidades, ganchos). */
export function estadoAgora(conversa: Conversation | null, ultimaAcao: string | null = null): EstadoAgora {
  const capacidades = conversa?.capabilities ?? null;
  if (capacidades?.pedido) return { tom: "precisa", rotulo: "Esperando sua aprovação", detalhe: emUmaLinha(capacidades.pedido.resumo, 120) };
  if (capacidades?.approve || conversa?.state === "approval_required") {
    return { tom: "precisa", rotulo: "Esperando sua aprovação", detalhe: emUmaLinha(capacidades?.approval_text, 120) };
  }
  if (capacidades?.escolha) {
    return { tom: "precisa", rotulo: capacidades.escolha.da_cli ? "A CLI está perguntando" : "Perguntou algo para você", detalhe: emUmaLinha(capacidades.escolha.pergunta, 140) };
  }
  if (capacidades?.espera) return { tom: "precisa", rotulo: "Esperando você no terminal", detalhe: emUmaLinha(capacidades.espera.texto, 120) };
  if (conversa?.notice === "usage_limit") return { tom: "erro", rotulo: "Parou no limite de uso", detalhe: null };
  if (conversa?.notice === "api_error") return { tom: "erro", rotulo: "Parou num erro da API", detalhe: null };
  if (conversa?.state === "crashed") return { tom: "erro", rotulo: "A sessão caiu", detalhe: null };
  if (conversa?.state === "working") {
    return { tom: "trabalhando", rotulo: "Trabalhando", detalhe: emUmaLinha(conversa.ganchos?.ferramenta, 90) ?? ultimaAcao };
  }
  if (conversa?.state === "answered") return { tom: "pronto", rotulo: "Terminou, esperando sua próxima mensagem", detalhe: null };
  if (conversa?.state === "stopped" || conversa?.state === "orphan") return { tom: "parado", rotulo: "Sessão encerrada", detalhe: null };
  return { tom: "parado", rotulo: conversa?.live === false ? "Sem sessão aberta" : "Sem estado ainda", detalhe: null };
}

function resumoDoAgente(conversa: Conversation | null): ResumoDoAgente | null {
  const bruto = (conversa as (Conversation & { resumo_do_agente?: unknown }) | null)?.resumo_do_agente;
  if (!bruto || typeof bruto !== "object" || Array.isArray(bruto)) return null;
  const campo = (nome: string) => {
    const valor = (bruto as Record<string, unknown>)[nome];
    return typeof valor === "string" && valor.trim() ? emUmaLinha(valor, 140) ?? undefined : undefined;
  };
  const extra: ResumoDoAgente = { fazendo: campo("fazendo"), proximo: campo("proximo"), bloqueio: campo("bloqueio") };
  const quando = (bruto as Record<string, unknown>).atualizado_em_ms;
  if (typeof quando === "number") extra.atualizado_em_ms = quando;
  return extra.fazendo || extra.proximo || extra.bloqueio ? extra : null;
}

/* ── Montagem ────────────────────────────────────────────────────────────────────────────────── */

export interface EntradaDoCartao {
  /** A página da timeline que a tela tem (as mais recentes). */
  mensagens: Mensagem[];
  /** A lista de tarefas atual (`timeline.tarefas`). */
  tarefas?: Tarefa[] | null;
  conversa: Conversation | null;
  olhada: Olhada | null;
  agora: number;
  /** Fronteira congelada na volta (número da última mensagem vista). Sem ela, vale a da olhada. */
  desde?: number | null;
}

export function montarOndeParei({ mensagens, tarefas, conversa, olhada, agora, desde }: EntradaDoCartao): OndeParei {
  const fronteira = desde ?? olhada?.n ?? null;
  const ultimoPedidoIdx = (() => {
    for (let i = mensagens.length - 1; i >= 0; i -= 1) {
      const m = mensagens[i];
      if (m.role === "user" && (m.kind ?? "text") === "text" && m.text.trim()) return i;
    }
    return -1;
  })();
  const pedido = ultimoPedidoIdx >= 0 ? emUmaLinha(mensagens[ultimoPedidoIdx].text, 140) : null;
  const novasMensagens = fronteira === null ? [] : mensagens.filter((m) => indiceDe(m) > fronteira);
  const janela: OndeParei["janela"] = novasMensagens.length > 0 ? "olhada" : "pedido";
  const daJanela = janela === "olhada" ? novasMensagens : mensagens.slice(ultimoPedidoIdx + 1);
  const feitos = contarFeitos(daJanela);
  let ultimaFala: string | null = null;
  for (let i = daJanela.length - 1; i >= 0 && !ultimaFala; i -= 1) {
    const m = daJanela[i];
    if (m.role === "assistant" && (m.kind ?? "text") === "text") ultimaFala = emUmaLinha(m.text, 160);
  }
  const lista = tarefas ?? null;
  const andamento = lista?.find((t) => t.estado === "andamento");
  const plano = lista && lista.length > 0
    ? { feitas: lista.filter((t) => t.estado === "feita").length, total: lista.length, fazendo: andamento ? emUmaLinha(andamento.fazendo ?? andamento.texto, 80) : null }
    : null;
  const primeiro = mensagens[0] ? indiceDe(mensagens[0]) : null;
  return {
    provider: conversa?.provider ?? mensagens[mensagens.length - 1]?.provider ?? null,
    estado: estadoAgora(conversa, feitos.ultimaAcao),
    pedido,
    janela,
    novas: novasMensagens.length,
    truncado: fronteira !== null && primeiro !== null && primeiro > fronteira + 1 && novasMensagens.length > 0,
    primeiroNovo: novasMensagens[0] ? indiceDe(novasMensagens[0]) : null,
    feitos,
    plano,
    ultimaFala,
    ausenciaMs: olhada ? Math.max(0, agora - olhada.em) : null,
    extra: resumoDoAgente(conversa),
  };
}

/** Maior número de mensagem da página (o que "já vi" grava). */
export function ultimoIndice(mensagens: Mensagem[]): number {
  return mensagens.reduce((maior, m) => Math.max(maior, indiceDe(m)), -1);
}

/* ── Quando mostrar ──────────────────────────────────────────────────────────────────────────── */

/** Voltou depois disto e chegou coisa nova (ou o estado mudou): mostra. */
export const AUSENCIA_CURTA_MS = 30_000;
/** Voltou depois disto, mesmo sem novidade: mostra para reorientar. */
export const AUSENCIA_LONGA_MS = 15 * 60_000;

export function deveMostrar(cartao: OndeParei, olhada: Olhada | null): boolean {
  if (!cartao.pedido && cartao.novas === 0) return false; // conversa sem nada ainda
  if (!olhada) return Boolean(cartao.pedido); // nunca olhou nesta versão: um resumo para começar
  const ausencia = cartao.ausenciaMs ?? 0;
  const mudouEstado = Boolean(olhada.tom) && olhada.tom !== cartao.estado.tom;
  if ((cartao.novas > 0 || mudouEstado) && ausencia >= AUSENCIA_CURTA_MS) return true;
  return ausencia >= AUSENCIA_LONGA_MS;
}

/* ── Textos do cartão e da lista ─────────────────────────────────────────────────────────────── */

/** Os "feitos" em pedaços curtos: "3 arquivos (+42 −8)", "5 comandos (1 falhou)", "Testes falharam". */
export function pedacosDosFeitos(feitos: Feitos): { texto: string; tom?: "ok" | "falha" }[] {
  const pedacos: { texto: string; tom?: "ok" | "falha" }[] = [];
  const editados = feitos.arquivos.length + feitos.edicoesSemArquivo;
  if (feitos.testes) {
    pedacos.push(feitos.testes.ok === false ? { texto: "Testes falharam", tom: "falha" }
      : feitos.testes.ok === true ? { texto: "Testes passaram", tom: "ok" } : { texto: "Rodou os testes" });
  }
  if (editados > 0) {
    const linhas = feitos.adicionadas || feitos.removidas ? ` (+${feitos.adicionadas} −${feitos.removidas})` : "";
    const texto = editados === 1
      ? (feitos.arquivos[0] ? `Editou ${feitos.arquivos[0].nome}${linhas}` : "1 edição")
      : `${editados} ${feitos.arquivos.length ? "arquivos" : "edições"}${linhas}`;
    pedacos.push({ texto });
  }
  if (feitos.comandos > 0) {
    const falhas = feitos.comandosQueFalharam > 0 ? ` (${feitos.comandosQueFalharam} ${feitos.comandosQueFalharam === 1 ? "falhou" : "falharam"})` : "";
    pedacos.push({ texto: `${feitos.comandos} ${feitos.comandos === 1 ? "comando" : "comandos"}${falhas}`, tom: feitos.comandosQueFalharam > 0 && !feitos.testes ? "falha" : undefined });
  }
  const consultas = feitos.leituras + feitos.buscas;
  if (consultas > 0) pedacos.push({ texto: `${consultas} ${consultas === 1 ? "leitura" : "leituras"}` });
  return pedacos;
}

/** A linha de cada agente na lista de troca rápida e no "Na tela": o estado e o essencial. */
export function linhaEssencial(estado: EstadoAgora, cartao: Pick<OndeParei, "ultimaFala" | "feitos"> | null): string {
  switch (estado.tom) {
    case "precisa":
      if (estado.rotulo === "Esperando sua aprovação") return estado.detalhe ? `Aprovar: ${estado.detalhe}` : estado.rotulo;
      return estado.detalhe ? `${estado.rotulo.startsWith("Perguntou") ? "Pergunta" : "Espera"}: ${estado.detalhe}` : estado.rotulo;
    case "erro":
      return estado.rotulo;
    case "trabalhando":
      return estado.detalhe ? `Trabalhando · ${estado.detalhe}` : "Trabalhando";
    case "pronto": {
      const testes = cartao?.feitos.testes;
      const prefixo = testes?.ok === false ? "Terminou (testes falharam)" : "Terminou";
      return cartao?.ultimaFala ? `${prefixo}: ${cartao.ultimaFala}` : prefixo;
    }
    default:
      return estado.rotulo;
  }
}

/** Ordem da lista de troca rápida: o que pede você primeiro. */
export const ORDEM_DO_TOM: readonly Tom[] = ["precisa", "erro", "pronto", "trabalhando", "parado"];
