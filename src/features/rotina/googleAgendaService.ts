// Fork (Google Agenda): conversa da tela com o engine sobre o Google Agenda pessoal
// (`crates/omni-engine/src/mobile/google_agenda.rs`). Conectar e desconectar só existem no PC.
import { engineApi } from "../terminal/terminalService";

export interface GoogleAgenda {
  conectado: boolean;
  conectando: boolean;
  conta: string;
  sincronizado_ms: number;
  eventos: number;
  erro: string | null;
  /** Só na resposta de "conectar": a página de login do Google para abrir no navegador. */
  url?: string;
}

export interface NovoCompromisso { titulo: string; data: string; inicio: string; fim: string; local: string; notas: string }

async function pedir<T>(metodo: "GET" | "POST", caminho: string, corpo?: unknown): Promise<T> {
  const resposta = await engineApi(metodo, caminho, corpo === undefined ? null : JSON.stringify(corpo));
  if (!resposta || resposta.status >= 400) {
    let erro = resposta?.status === 404 ? "Atualize o Niski: este engine ainda não tem o Google Agenda." : "O Google Agenda não respondeu. Tente de novo.";
    try { erro = (JSON.parse(resposta?.body ?? "") as { error?: string }).error ?? erro; } catch { /* mantém o padrão */ }
    throw new Error(erro);
  }
  return JSON.parse(resposta.body) as T;
}

export const lerGoogleAgenda = () => pedir<GoogleAgenda>("GET", "/rotina/google");
export const conectarGoogleAgenda = (client_id: string, client_secret: string) =>
  pedir<GoogleAgenda>("POST", "/rotina/google/conectar", { client_id, client_secret });
export const desconectarGoogleAgenda = () => pedir<GoogleAgenda>("POST", "/rotina/google/desconectar", {});
export const sincronizarGoogleAgenda = () => pedir<{ ok: boolean; eventos: number }>("POST", "/rotina/google/sincronizar", {});
export const criarCompromisso = (c: NovoCompromisso) => pedir<{ ok: boolean; id: string }>("POST", "/rotina/google/evento", c);
