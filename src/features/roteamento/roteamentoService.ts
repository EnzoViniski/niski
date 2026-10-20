import { request } from "../../mobile/api";

/**
 * Fork (3B): fila de contas. A regra mora no engine (`omni_core::roteamento`); aqui só os tipos das
 * rotas `/roteamento` e o texto da cota. Preço nenhum: orçamento e gasto são números que o usuário digita.
 */

/** Valor de conta que pede a escolha pela fila (Novo agente, `criar_agente`, modelo de agente). */
export const CONTA_AUTOMATICA = "auto";

export type TipoDeConta = "assinatura" | "api";
export type ModoDaFila = "fila" | "auto";

export interface AjustesDaConta {
  tipo: TipoDeConta;
  plano?: string | null;
  uso_extra?: boolean;
  orcamento_usd?: number | null;
  gasto_usd?: number;
  expira_em_ms?: number | null;
  max_concorrencia?: number | null;
  confidencial_ok?: boolean;
}

export interface Politica {
  modo: ModoDaFila;
  etapas: string[];
  contas: Record<string, AjustesDaConta>;
  projetos_confidenciais: string[];
  usar_em_fluxos: boolean;
}

export interface JanelaDeCota { nome: string; percentual: number; reset_em_ms: number | null }
export interface CotaDaConta { conhecida: boolean; fonte: string | null; observado_em_ms: number | null; janelas: JanelaDeCota[] }

export interface ContaDaFila {
  id: string;
  nome: string;
  provider: string;
  na_fila: boolean;
  ajustes: AjustesDaConta;
  /** Sessões de agente abertas agora nesta conta. */
  sessoes: number;
  cota: CotaDaConta;
}

export interface EscolhaDaFila {
  perfil_id: string | null;
  nome: string | null;
  /** Só no aviso de sessão interativa após falha: a conta que a fila sugere (nada troca sozinho). */
  sugestao?: string | null;
  nome_da_sugestao?: string | null;
  /** Uma linha por conta avaliada: por que foi pulada e por que a escolhida venceu. */
  explicacao: string[];
}

export interface EstadoDaFila {
  politica: Politica;
  contas: ContaDaFila[];
  /** "Por que esta conta" de cada provider que tem conta cadastrada. */
  escolhas: Record<string, EscolhaDaFila>;
  agora_ms: number;
}

const JSON_POST = { method: "POST", headers: { "Content-Type": "application/json" } } as const;

export function lerFila() {
  return request<EstadoDaFila>("/roteamento");
}

export function gravarFila(politica: Politica) {
  return request<EstadoDaFila>("/roteamento", { ...JSON_POST, body: JSON.stringify(politica) });
}

/** A conta que "Automática" usaria agora para `provider` neste projeto, com o porquê. */
export function escolherConta(provider: string, projeto?: string) {
  return request<EscolhaDaFila>("/roteamento/escolher", { ...JSON_POST, body: JSON.stringify({ provider, projeto }) });
}

const FONTES: Record<string, string> = {
  claude_statusline: "statusline do Claude",
  claude_cache: "cache do /usage do Claude",
  claude_screen: "tela do /usage",
  codex_rollout: "registro do Codex",
  codex_app_server: "app-server do Codex",
  antigravity_screen: "tela do Antigravity",
};

export const nomeDaFonte = (fonte: string | null) => (fonte ? FONTES[fonte] ?? fonte : "sem fonte");

/** "há 3 min", "há 2 h", "idade desconhecida": dado velho decide mal, então a tela sempre diz a idade. */
export function idadeDoDado(observadoEm: number | null, agora: number): string {
  if (observadoEm === null) return "idade desconhecida";
  const minutos = Math.max(0, Math.floor((agora - observadoEm) / 60_000));
  if (minutos < 60) return `há ${minutos} min`;
  const horas = Math.floor(minutos / 60);
  return horas < 48 ? `há ${horas} h` : `há ${Math.floor(horas / 24)} dias`;
}

/** "5h 30% · semanal 70% · statusline do Claude, há 2 min" ou "desconhecida · registro do Codex, idade desconhecida". */
export function resumoDaCota(cota: CotaDaConta, agora: number): string {
  const origem = `${nomeDaFonte(cota.fonte)}, ${idadeDoDado(cota.observado_em_ms, agora)}`;
  if (!cota.conhecida) return cota.fonte ? `cota desconhecida · ${origem}` : "cota desconhecida · sem fonte";
  const janelas = cota.janelas.map((janela) =>
    janela.reset_em_ms !== null && janela.reset_em_ms <= agora ? `${janela.nome} reiniciada` : `${janela.nome} ${Math.round(janela.percentual)}%`);
  return [...janelas, origem].join(" · ");
}

/** Move a conta `id` uma posição na fila (`-1` sobe, `1` desce). */
export function moverNaFila(etapas: string[], id: string, passo: -1 | 1): string[] {
  const de = etapas.indexOf(id);
  const para = de + passo;
  if (de < 0 || para < 0 || para >= etapas.length) return etapas;
  const nova = [...etapas];
  [nova[de], nova[para]] = [nova[para], nova[de]];
  return nova;
}
