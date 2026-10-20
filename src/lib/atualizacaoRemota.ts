import { useEffect, useSyncExternalStore } from "react";
import { ATUALIZACAO_AUTOMATICA } from "./identidade";
import { espelharAgora } from "./espelhoDaInterface";

/*
 * Fork (atualização automática): versão nova publicada no repositório de releases → "Atualizar agora".
 *
 * Um estado só, compartilhado pela barra (`AvisoDeAtualizacao`), por Configurações (`UpdateCard`) e
 * pelo sino (`useNotifications`): uma checagem ao abrir e outra a cada 30 min, falha de rede calada.
 *
 * Atualizar = cópia do estado (`backup_antes_de_atualizar`) → baixar e instalar (o plugin confere a
 * assinatura minisign antes) → `reiniciar_para_atualizar` com `trocarEngine`: o engine grava as
 * sessões e sai, a janela reabre com o `.app` novo e sobe o engine novo. Não usa o `relaunch()` do
 * plugin-process: ele reabriria a janela falando com o engine antigo. Um sinalizador no localStorage
 * diz à próxima abertura que foi um reinício de atualização (`consumirReinicioDeAtualizacao`).
 *
 * Imports do plugin e do `invoke` são dinâmicos: fora do Tauri (`dev:vite`, testes) eles não existem.
 */

export const INTERVALO_DA_CHECAGEM_MS = 30 * 60 * 1000;
export const CHAVE_DO_REINICIO = "medza-reinicio-de-atualizacao";
/** Sinalizador mais velho que isto é sobra (o reinício não aconteceu): ignorado. */
const VALIDADE_DO_REINICIO_MS = 15 * 60 * 1000;
/** O que a interface guarda no localStorage e vai junto na cópia (o Rust filtra pela mesma lista). */
const CHAVES_DA_INTERFACE = [
  "omni-agents-workspaces",
  "omni-agents-workspace",
  "omni-agents-settings",
  "omni-agents-kanban",
  "omni-agents-recent-projects",
  "medza-escolhas",
];

export type FaseRemota = "ocioso" | "checando" | "atual" | "disponivel" | "baixando" | "instalada" | "reiniciando" | "erro";

export interface EstadoRemoto {
  fase: FaseRemota;
  versao: string | null;
  notas: string | null;
  /** 0–100 durante o download. */
  progresso: number;
  erro: string | null;
}

interface Pacote {
  version: string;
  body?: string;
  downloadAndInstall: (aoProgredir?: (evento: EventoDoDownload) => void) => Promise<void>;
}

type EventoDoDownload =
  | { event: "Started"; data: { contentLength?: number } }
  | { event: "Progress"; data: { chunkLength: number } }
  | { event: "Finished" };

const INICIAL: EstadoRemoto = { fase: "ocioso", versao: null, notas: null, progresso: 0, erro: null };

let estado: EstadoRemoto = INICIAL;
let pacote: Pacote | null = null;
const ouvintes = new Set<() => void>();
let usuarios = 0;
let relogio: number | null = null;

function mudar(parcial: Partial<EstadoRemoto>) {
  estado = { ...estado, ...parcial };
  ouvintes.forEach((ouvinte) => ouvinte());
}

const ocupado = () => estado.fase === "baixando" || estado.fase === "instalada" || estado.fase === "reiniciando";

async function chamar<T>(comando: string, args?: Record<string, unknown>): Promise<T> {
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<T>(comando, args);
}

function mensagem(erro: unknown) {
  return erro instanceof Error ? erro.message : String(erro);
}

export function estadoDaAtualizacaoRemota(): EstadoRemoto {
  return estado;
}

/** Só para testes: volta ao começo. */
export function redefinirAtualizacaoRemota() {
  estado = INICIAL;
  pacote = null;
  usuarios = 0;
  if (relogio !== null) window.clearInterval(relogio);
  relogio = null;
  ouvintes.forEach((ouvinte) => ouvinte());
}

/** Procura versão nova. `silencioso`: sem "procurando…" e sem erro na tela (checagem automática). */
export async function procurarAtualizacao({ silencioso = false } = {}): Promise<void> {
  if (!ATUALIZACAO_AUTOMATICA || ocupado()) return;
  if (!silencioso) mudar({ fase: "checando", erro: null });
  try {
    const { check } = await import("@tauri-apps/plugin-updater");
    const achado = (await check()) as Pacote | null;
    if (ocupado()) return;
    pacote = achado;
    if (achado) {
      mudar({ fase: "disponivel", versao: achado.version, notas: achado.body ?? null, erro: null });
      // Só no build de teste do updater (scripts/build-mac-local.mjs com MEDZA_TESTE_AUTOATUALIZAR=1):
      // clica sozinho em "Atualizar agora" para o teste de ponta a ponta sem mão humana.
      if (import.meta.env.VITE_MEDZA_TESTE_AUTOATUALIZAR === "1") void atualizarAgora();
    } else if (!silencioso) mudar({ fase: "atual", versao: null, notas: null });
    else if (estado.fase === "disponivel") mudar({ fase: "ocioso", versao: null, notas: null });
  } catch (erro) {
    // Sem internet no boot não é assunto do usuário.
    if (silencioso) return;
    mudar({ fase: "erro", erro: mensagem(erro) });
  }
}

/** Checagem ao abrir e a cada 30 min enquanto alguém (barra, sino) estiver olhando. */
export function iniciarChecagemAutomatica(): () => void {
  if (!ATUALIZACAO_AUTOMATICA) return () => undefined;
  usuarios += 1;
  if (usuarios === 1) {
    void procurarAtualizacao({ silencioso: true });
    relogio = window.setInterval(() => void procurarAtualizacao({ silencioso: true }), INTERVALO_DA_CHECAGEM_MS);
  }
  return () => {
    usuarios = Math.max(0, usuarios - 1);
    if (usuarios === 0 && relogio !== null) {
      window.clearInterval(relogio);
      relogio = null;
    }
  };
}

/** Agentes trabalhando agora (seriam interrompidos e voltam com `--resume`). Falha conta como 0. */
export async function agentesTrabalhando(): Promise<number> {
  try {
    return (await chamar<number>("agentes_trabalhando")) ?? 0;
  } catch {
    return 0;
  }
}

/** Texto da confirmação do "Atualizar agora". */
export function textoDaConfirmacao(versao: string | null, ocupados: number): string {
  const qual = versao ? `a versão ${versao}` : "a versão nova";
  const agentes = ocupados === 0 ? ""
    : ocupados === 1 ? " 1 agente trabalhando agora será interrompido e volta com --resume (a mesma conversa)."
    : ` ${ocupados} agentes trabalhando agora serão interrompidos e voltam com --resume (a mesma conversa).`;
  return `O app baixa ${qual}, instala e reabre sozinho. As conversas e abas voltam como estavam.${agentes}`;
}

export function marcarReinicioDeAtualizacao() {
  try {
    localStorage.setItem(CHAVE_DO_REINICIO, JSON.stringify({ em: Date.now() }));
  } catch {
    /* sem localStorage: a abertura segue normal */
  }
}

function desmarcarReinicioDeAtualizacao() {
  try {
    localStorage.removeItem(CHAVE_DO_REINICIO);
  } catch {
    /* idem */
  }
}

/** Uma vez por abertura: `true` se esta abertura veio de um reinício de atualização. */
export function consumirReinicioDeAtualizacao(agora = Date.now()): boolean {
  try {
    const bruto = localStorage.getItem(CHAVE_DO_REINICIO);
    if (!bruto) return false;
    localStorage.removeItem(CHAVE_DO_REINICIO);
    const em = Number((JSON.parse(bruto) as { em?: unknown }).em);
    return Number.isFinite(em) && agora - em >= 0 && agora - em <= VALIDADE_DO_REINICIO_MS;
  } catch {
    return false;
  }
}

function interfaceParaOBackup(): Record<string, string> {
  const salvas: Record<string, string> = {};
  for (const chave of CHAVES_DA_INTERFACE) {
    try {
      const valor = localStorage.getItem(chave);
      if (valor !== null) salvas[chave] = valor;
    } catch {
      /* sem localStorage */
    }
  }
  return salvas;
}

/** Reabre com o engine novo: o engine grava as sessões e sai; cada aba volta com `--resume`. */
export async function reiniciarParaAtualizar(): Promise<void> {
  marcarReinicioDeAtualizacao();
  // O reinício sai sem esperar o WebKit gravar: a cópia em disco leva as últimas mudanças.
  await espelharAgora().catch(() => undefined);
  mudar({ fase: "reiniciando", erro: null });
  try {
    await chamar("reiniciar_para_atualizar", { trocarEngine: true });
  } catch (erro) {
    desmarcarReinicioDeAtualizacao();
    // Já instalada: o botão vira "Reabrir agora".
    mudar({ fase: "instalada", erro: mensagem(erro) });
  }
}

/** Cópia do estado → baixar e instalar (com progresso) → reabrir. */
export async function atualizarAgora(): Promise<void> {
  if (!ATUALIZACAO_AUTOMATICA || ocupado()) return;
  mudar({ fase: "baixando", progresso: 0, erro: null });

  // A cópia é a rede de segurança, não condição: o updater não toca na pasta de dados.
  try {
    await chamar("backup_antes_de_atualizar", { interface: interfaceParaOBackup() });
  } catch (erro) {
    void chamar("registrar_evento_da_ui", { evento: "backup_de_atualizacao", detalhe: mensagem(erro).slice(0, 200) }).catch(() => undefined);
  }

  try {
    let atual = pacote;
    if (!atual) {
      const { check } = await import("@tauri-apps/plugin-updater");
      atual = (await check()) as Pacote | null;
    }
    if (!atual) {
      mudar({ fase: "atual", versao: null, notas: null });
      return;
    }
    let total = 0;
    let baixado = 0;
    // No Windows o instalador fecha o app no meio: o sinalizador já tem de estar gravado.
    marcarReinicioDeAtualizacao();
    await atual.downloadAndInstall((evento) => {
      if (evento.event === "Started") total = evento.data.contentLength ?? 0;
      else if (evento.event === "Progress") {
        baixado += evento.data.chunkLength;
        if (total > 0) mudar({ progresso: Math.min(100, Math.round((baixado / total) * 100)) });
      } else if (evento.event === "Finished") mudar({ progresso: 100 });
    });
  } catch (erro) {
    desmarcarReinicioDeAtualizacao();
    mudar({ fase: "erro", erro: mensagem(erro) });
    return;
  }
  await reiniciarParaAtualizar();
}

function assinar(ouvinte: () => void) {
  ouvintes.add(ouvinte);
  return () => {
    ouvintes.delete(ouvinte);
  };
}

/** Estado compartilhado. `automatica`: liga a checagem ao abrir e a cada 30 min. */
export function useAtualizacaoRemota({ automatica = false } = {}): EstadoRemoto {
  useEffect(() => (automatica ? iniciarChecagemAutomatica() : undefined), [automatica]);
  return useSyncExternalStore(assinar, estadoDaAtualizacaoRemota, estadoDaAtualizacaoRemota);
}
