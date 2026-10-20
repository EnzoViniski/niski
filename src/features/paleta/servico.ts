import { engineApi } from "../terminal/terminalService";
import type { Conversation, Painel } from "../../mobile/api";

export interface AchadoDoHistorico {
  conversa_id: string; titulo: string; papel: string; trecho: string; data: string | null;
}
export interface BuscaNoHistorico { achados: AchadoDoHistorico[]; parcial: boolean }
async function ler<T>(metodo: string, rota: string, corpo?: string): Promise<T> {
  const resposta = await engineApi(metodo, rota, corpo ?? null);
  if (resposta.status >= 400) throw new Error("Não foi possível consultar o histórico.");
  return JSON.parse(resposta.body) as T;
}
export async function listarConversas(): Promise<Conversation[]> {
  return (await ler<Painel>("GET", "/painel")).conversations;
}
export function buscarNoHistorico(texto: string): Promise<BuscaNoHistorico> {
  // Texto no corpo: não entra em URLs, logs de acesso ou mensagens de erro.
  return ler("POST", "/historico/buscar", JSON.stringify({ texto, limite: 40, tempo_ms: 750 }));
}
