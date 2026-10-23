// Fork (Rotina): transporte da tela Rotina para o engine. Leitura em `GET /rotina/ler/<operação>`,
// mudança em `POST /rotina/op/<operação>` (as mesmas operações que o agente da rotina usa pelo MCP),
// a conversa do agente em `/rotina/conversa` e o bot da rotina em `/rotina/telegram` (só o desktop).
import { engineApi } from "../terminal/terminalService";
import type { AjustesDaRotina, Alimentacao, Avisos, ConversaDaRotina, Dia, Lembretes, SemanaDaRotina, TelegramDaRotina, Treino } from "./tipos";

function corpoDoErro(body: string, padrao: string): string {
  try { return (JSON.parse(body) as { error?: string }).error ?? padrao; } catch { return padrao; }
}

/** Fork (Rotina no celular): como o pedido chega ao engine. No app do PC, pelo IPC; no celular, pela rede. */
export type TransporteDaRotina = (metodo: "GET" | "POST", caminho: string, corpo: string | null) => Promise<{ status: number; body: string } | null>;
let transporte: TransporteDaRotina = (metodo, caminho, corpo) => engineApi(metodo, caminho, corpo);
export function usarTransporteDaRotina(novo: TransporteDaRotina) { transporte = novo; }

async function pedir<T>(metodo: "GET" | "POST", caminho: string, corpo?: unknown): Promise<T> {
  const resposta = await transporte(metodo, caminho, corpo === undefined ? null : JSON.stringify(corpo));
  if (!resposta || resposta.status >= 400) {
    const padrao = resposta?.status === 404 ? "Atualize o Niski: este engine ainda não tem a Rotina." : "A rotina não respondeu. Tente de novo.";
    throw new Error(corpoDoErro(resposta?.body ?? "", padrao));
  }
  return JSON.parse(resposta.body) as T;
}

function consulta(args: Record<string, string | number | undefined>): string {
  const p = new URLSearchParams();
  for (const [k, v] of Object.entries(args)) if (v !== undefined && v !== "") p.set(k, String(v));
  const q = p.toString();
  return q ? `?${q}` : "";
}

export const lerDia = (data?: string) => pedir<Dia>("GET", `/rotina/ler/ler_dia${consulta({ data })}`);
export const lerSemana = (semana?: string) => pedir<SemanaDaRotina>("GET", `/rotina/ler/ler_semana${consulta({ semana })}`);
export const lerTreino = (exercicio?: string, semanas = 12) => pedir<Treino>("GET", `/rotina/ler/ler_treino${consulta({ exercicio, semanas, sessoes: 60 })}`);
export const lerAlimentacao = (data?: string, dias = 14) => pedir<Alimentacao>("GET", `/rotina/ler/ler_alimentacao${consulta({ data, dias })}`);
export const lerLembretes = () => pedir<Lembretes>("GET", "/rotina/ler/ler_lembretes");
export const lerAjustes = () => pedir<AjustesDaRotina>("GET", "/rotina/ler/ler_ajustes");
export const lerAvisos = () => pedir<Avisos>("GET", "/rotina/ler/ler_avisos");

/** Qualquer operação que muda a rotina (`omni_core::rotina::operacoes`). */
export const operar = <T = Record<string, unknown>>(nome: string, args: Record<string, unknown> = {}) =>
  pedir<T>("POST", `/rotina/op/${nome}`, args);

export const conversaAtual = () => pedir<ConversaDaRotina>("GET", "/rotina/conversa");
export const abrirConversa = (modo: "planejar" | "conversar", padrao?: { plataforma?: string; conta?: string }) =>
  pedir<ConversaDaRotina & { nova: boolean }>("POST", "/rotina/conversa", {
    modo, ...(padrao?.plataforma ? { plataforma_padrao: padrao.plataforma } : {}), ...(padrao?.conta ? { conta_padrao: padrao.conta } : {}),
  });

export const telegramDaRotina = (caminho = "", corpo?: unknown) =>
  pedir<TelegramDaRotina>(corpo === undefined ? "GET" : "POST", `/rotina/telegram${caminho}`, corpo);
