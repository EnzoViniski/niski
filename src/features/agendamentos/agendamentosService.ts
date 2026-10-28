// O engine é a fonte única, inclusive com a janela do app fechada.
import { request } from "../../mobile/api";
import type { Conversation } from "../../mobile/api";
export type Regra = { tipo: "uma_vez"; em_ms: number } | { tipo: "intervalo"; minutos: number }
  | { tipo: "diario"; horario: string } | { tipo: "semanal"; horario: string; dias: number[] };
export type Destino = { tipo: "conversa"; conversa: string } | { tipo: "novo_agente"; projeto: string; plataforma: string;
  conta: string | null; modelo: string | null; pasta: string; worktree: boolean };
export interface Pedido { id?: string; titulo: string; prompt: string; regra: Regra; destino: Destino }
export interface Agendamento extends Pedido { id: string; pausado: boolean; proxima_ms: number | null; fuso: string;
  historico: { em_ms: number; estado: string; conversa: string | null }[]; pendente: unknown | null }
export interface Opcoes { projetos: { id: string; name: string; path: string }[]; fuso: string;
  contas: { id: string; name: string; provider: string }[]; plataformas: { id: string; label: string }[] }
const post = <T>(path: string, body?: unknown) => request<T>(path, { method: "POST", headers: { "Content-Type": "application/json" }, ...(body ? { body: JSON.stringify(body) } : {}) });
export const listarAgendamentos = () => request<Agendamento[]>("/agendamentos");
export const opcoesAgendamentos = () => request<Opcoes>("/agendamentos/opcoes");
export const conversasAgendaveis = () => request<Conversation[]>("/conversas");
export const salvarAgendamento = (pedido: Pedido) => post("/agendamentos", pedido);
export const agirNoAgendamento = (id: string, acao: "rodar" | "pausar" | "retomar" | "apagar") => post(`/agendamentos/${encodeURIComponent(id)}/${acao}`);
export const resolverDataLocal = (local: string) => post<{ em_ms: number }>("/agendamentos/data", { local });
export function abrirAgendamentos() { window.dispatchEvent(new Event("medza-abrir-agendamentos")); }
export const ESTADOS: Record<string, string> = { em_andamento: "Em andamento", adiado: "Adiado: aguardando vaga ou campo livre", enviado: "Prompt enviado à CLI",
  erro: "Falhou ao executar", interrompido: "Interrompido pelo reinício do engine; confira a conversa antes de rodar", expirado: "Expirou após 24 horas de espera" };
