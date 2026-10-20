import { engineApi } from "../terminal/terminalService";

/*
 * Fork (retomar-sempre): retomar uma conversa nunca termina num beco sem saída. As rotas
 * `/retomada/*` do engine (`crates/omni-engine/src/mobile/retomada_rotas.rs`) dizem se a conversa
 * já está viva numa aba, se a pasta ainda existe (e qual é o repositório principal) e em que conta
 * está o transcript; e abrem uma conversa nova com o histórico como contexto quando a CLI não
 * retoma. Engine antigo (troca adiada pela atualização) não tem as rotas: devolve 404 e quem chamou
 * segue pelo caminho de antes.
 */

export interface DiagnosticoDeRetomada {
  viva: { terminal_session_id: string; conversation_id?: string | null; project_id: string } | null;
  pasta_existe: boolean;
  pasta_principal: string | null;
  transcript: boolean;
  /** Conta onde a conversa está (ou onde foi preparada). */
  conta: string | null;
  /** O transcript já está onde a CLI da conta escolhida procura. */
  pronto: boolean;
  /** Conserva o servidor local da conversa ao retomar pelo Histórico. */
  modelo_local?: { provedor: "ollama" | "lmstudio"; modelo: string } | null;
}

export interface PedidoDeRetomada {
  provider: string;
  sessionId: string;
  profileId?: string | null;
  cwd?: string | null;
}

/** Uma saída que a tela oferece quando a retomada não pode seguir como pedida. */
export interface AcaoDeRetomada {
  rotulo: string;
  dica?: string;
  opcoes: OpcoesDeRetomada;
}

export interface OpcoesDeRetomada {
  /** Abrir em outra pasta (a da conversa sumiu). */
  cwd?: string;
  /** Desistir do `--resume`: conversa nova com as últimas mensagens da antiga como contexto. */
  continuarNova?: boolean;
}

/** Erro com saídas: a tela mostra a mensagem e um botão para cada ação. */
export class RetomadaError extends Error {
  constructor(message: string, public readonly acoes: AcaoDeRetomada[] = []) { super(message); }
}

export const ROTA_AUSENTE = 404;

function corpoDoErro(body: string): { error?: string; codigo?: string; pasta_principal?: string | null } {
  try { return JSON.parse(body) as { error?: string; codigo?: string; pasta_principal?: string | null }; } catch { return {}; }
}

/** `null` = engine sem a rota (antigo): siga pelo caminho de antes. */
export async function prepararRetomada(pedido: PedidoDeRetomada & { preparar?: boolean }): Promise<DiagnosticoDeRetomada | null> {
  const resposta = await engineApi("POST", "/retomada/preparar", JSON.stringify({
    provider: pedido.provider,
    session_id: pedido.sessionId,
    profile_id: pedido.profileId ?? null,
    cwd: pedido.cwd ?? null,
    preparar: pedido.preparar ?? false,
  }));
  if (resposta.status === ROTA_AUSENTE) return null;
  if (resposta.status >= 400) throw new Error(corpoDoErro(resposta.body).error ?? `Falha ao preparar a retomada (${resposta.status})`);
  return JSON.parse(resposta.body) as DiagnosticoDeRetomada;
}

export interface ConversaContinuada {
  conversation_id: string;
  terminal_session_id: string | null;
  /** O histórico foi achado e vai como primeira mensagem. */
  contexto: boolean;
}

export async function continuarEmConversaNova(pedido: PedidoDeRetomada & { cwd: string; projectId?: string; titulo?: string | null }): Promise<ConversaContinuada> {
  const resposta = await engineApi("POST", "/retomada/continuar-nova", JSON.stringify({
    provider: pedido.provider,
    session_id: pedido.sessionId,
    cwd: pedido.cwd,
    profile_id: pedido.profileId ?? null,
    project_id: pedido.projectId ?? null,
    titulo: pedido.titulo ? pedido.titulo.slice(0, 200) : null,
  }));
  if (resposta.status === ROTA_AUSENTE) throw new Error(MENSAGEM_ENGINE_ANTIGO);
  const corpo = corpoDoErro(resposta.body);
  if (resposta.status >= 400) {
    if (corpo.codigo === "pasta_sumiu") throw erroDePastaSumida(corpo.error ?? "A pasta desta conversa não existe mais.", corpo.pasta_principal ?? null, false);
    throw new Error(corpo.error ?? `Falha ao abrir a conversa nova (${resposta.status})`);
  }
  return JSON.parse(resposta.body) as ConversaContinuada;
}

/** Aba cuja pasta sumiu: reabre a mesma sessão em `cwd`. */
export async function reabrirEmOutraPasta(terminalSessionId: string, cwd: string): Promise<void> {
  const resposta = await engineApi("POST", "/retomada/mudar-pasta", JSON.stringify({ terminal_session_id: terminalSessionId, cwd }));
  if (resposta.status === ROTA_AUSENTE) throw new Error(MENSAGEM_ENGINE_ANTIGO);
  if (resposta.status >= 400) throw new Error(corpoDoErro(resposta.body).error ?? `Falha ao reabrir em outra pasta (${resposta.status})`);
}

export const MENSAGEM_ENGINE_ANTIGO =
  "O engine em execução é de uma versão anterior e não tem esta opção. Use \"Atualizar app e engine\" na barra de cima: as conversas voltam com o histórico.";

/** Pasta sumida → saídas: abrir no repositório principal (quando há sugestão) e, opcionalmente,
 *  continuar numa conversa nova lá. */
export function erroDePastaSumida(mensagem: string, pastaPrincipal: string | null, comContinuarNova = true): RetomadaError {
  const acoes: AcaoDeRetomada[] = [];
  if (pastaPrincipal) {
    acoes.push({ rotulo: "Abrir no repositório principal", dica: pastaPrincipal, opcoes: { cwd: pastaPrincipal } });
    if (comContinuarNova) acoes.push({ rotulo: "Conversa nova lá, com o histórico", dica: pastaPrincipal, opcoes: { cwd: pastaPrincipal, continuarNova: true } });
  }
  const sufixo = pastaPrincipal ? "" : " Abra o projeto de novo pela lateral e retome de lá.";
  return new RetomadaError(`${mensagem}${sufixo}`, acoes);
}

/** O erro do reinício quando a pasta sumiu (`retomada::PASTA_SUMIU` no engine). */
export function ehPastaSumida(erro: string): boolean {
  return erro.includes("PASTA_SUMIU");
}

/** Texto do erro de pasta sumida sem o código interno. */
export function semCodigo(erro: string): string {
  return erro.replace(/^.*PASTA_SUMIU:\s*/, "").replace(/^./, (letra) => letra.toUpperCase());
}

/** A CLI recusou o `--resume` (Claude Code: "No conversation found with session ID"). */
export function cliNaoAchouAConversa(saida: string): boolean {
  return /No conversation found with session ID/i.test(saida);
}
