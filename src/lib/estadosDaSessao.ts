/** Rótulo e tom de cada estado da sessão num lugar só. Antes cada tela tinha o seu mapa
 *  (celular, web, aba Conversa do PC, avisos). Os textos continuam os de cada tela — a web diz
 *  "Esperando você" onde o celular diz "Resposta disponível" — mas agora moram juntos, e um estado
 *  novo só precisa entrar aqui. */
import type { AttentionReason } from "../features/terminal/useAttention";
import type { TerminalState } from "../features/terminal/terminalService";

/** Lista e cabeçalho do celular. */
export const ROTULO_DO_ESTADO: Readonly<Record<TerminalState, string>> = {
  working: "Trabalhando",
  answered: "Resposta disponível",
  approval_required: "Aprovação pendente",
  stopped: "Encerrado",
  orphan: "Sessão anterior",
  crashed: "Interrompido",
};

/** Lista da web de computador: o mesmo do celular, menos o "pronto", que lá é "Esperando você". */
export const ROTULO_DO_ESTADO_NA_WEB: Readonly<Record<TerminalState, string>> = {
  ...ROTULO_DO_ESTADO,
  answered: "Esperando você",
};

/** Aba Conversa do PC, ao lado do título: frases mais longas, há espaço. */
export const ROTULO_DO_ESTADO_DETALHADO: Readonly<Record<TerminalState, string>> = {
  working: "Trabalhando",
  answered: "Pronta para a próxima mensagem",
  approval_required: "Esperando sua aprovação",
  stopped: "Sessão encerrada",
  orphan: "Sessão anterior",
  crashed: "A sessão caiu",
};

export type VarianteDoRotulo = "celular" | "web" | "detalhado";

const POR_VARIANTE: Record<VarianteDoRotulo, Readonly<Record<TerminalState, string>>> = {
  celular: ROTULO_DO_ESTADO,
  web: ROTULO_DO_ESTADO_NA_WEB,
  detalhado: ROTULO_DO_ESTADO_DETALHADO,
};

/** O rótulo do estado como o engine manda (`state`); `undefined` sem estado (conversa do histórico)
 *  ou estado desconhecido — cada tela escolhe o texto do "sem sessão". */
export function rotuloDoEstado(state: string | null | undefined, variante: VarianteDoRotulo = "celular"): string | undefined {
  const mapa = POR_VARIANTE[variante];
  return state && Object.prototype.hasOwnProperty.call(mapa, state) ? mapa[state as TerminalState] : undefined;
}

/** Cor do ponto de estado, na mesma linguagem do PC: trabalhando pulsa no acento, pronto é verde. */
export function tomDoEstado(state: string | null | undefined, aprovar: boolean): string {
  if (aprovar || state === "approval_required") return "m-dot-alerta";
  if (state === "working") return "m-dot-ativo";
  if (state === "answered") return "m-dot-ok";
  if (state === "crashed") return "m-dot-erro";
  return "m-dot-parado";
}

/** Por que um agente quer atenção, como os avisos e o painel de atenção escrevem. */
export const ROTULO_DA_ATENCAO: Readonly<Record<AttentionReason, string>> = {
  approval_required: "aprovação pendente",
  usage_limit: "limite de uso atingido",
  api_error: "erro de API",
  crashed: "travou",
  orphan: "sessão órfã",
  answered: "terminou",
  stopped: "parado",
};
