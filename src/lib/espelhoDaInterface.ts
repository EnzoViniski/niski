/*
 * Fork: o localStorage do WebKit guarda aparência, workspaces, abas e a ordem da lateral, mas perde
 * as últimas gravações quando o processo sai sem esperar o WebKit (reinício da atualização, app
 * derrubado) e some inteiro se a pasta do WebKit for limpa. Por isso a interface espelha essas chaves
 * num arquivo da pasta de dados (`src-tauri/src/estado_da_interface.rs`) e, ao abrir, restaura de lá
 * quando a cópia em disco é mais nova que a do localStorage.
 *
 * "Mais nova" é pelo carimbo `medza-interface-salva-em`, gravado no localStorage junto de cada
 * espelhamento: se o WebKit perdeu as últimas gravações, perdeu o carimbo junto, e o disco vence.
 * Fora do Tauri (`dev:vite`, testes) o `invoke` falha e tudo segue só com o localStorage.
 */

/** Mesma lista do Rust (`estado_da_interface::CHAVES`); o Rust descarta o que não estiver lá. */
export const CHAVES_ESPELHADAS = [
  "omni-agents-workspaces",
  "omni-agents-workspace",
  "omni-agents-workspaces.backup",
  "omni-agents-settings",
  "omni-agents-kanban",
  "omni-agents-recent-projects",
  "omni-ordem-lateral",
  "omni-pastas-abertas",
  "omni-lateral-largura",
  "omni-todo",
  "medza-escolhas",
  "medza-orquestradoras-recolhidas",
  "medza-manha", // Fork: rotina da manhã.
  "medza-tarefas-ajustes", // Fork: tarefas.
  "medza-tarefas-painel", // Fork: painel, botão e lista do dia flutuantes.
] as const;

export const CHAVE_DO_CARIMBO = "medza-interface-salva-em";
export const INTERVALO_DO_ESPELHO_MS = 2_000;
/** A abertura não espera mais que isto pelo disco. */
const ESPERA_MAXIMA_MS = 1_500;

export interface EstadoDaInterface {
  salvo_em: number;
  chaves: Record<string, string>;
}

export type Leitor = () => Promise<EstadoDaInterface | null>;
export type Gravador = (estado: EstadoDaInterface) => Promise<unknown>;

async function chamar<T>(comando: string, args?: Record<string, unknown>): Promise<T> {
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<T>(comando, args);
}

const lerDoDisco: Leitor = () => chamar<EstadoDaInterface | null>("ler_estado_da_interface");
const gravarNoDisco: Gravador = (estado) => chamar("gravar_estado_da_interface", { estado });

function lerChave(chave: string): string | null {
  try {
    return localStorage.getItem(chave);
  } catch {
    return null;
  }
}

function gravarChave(chave: string, valor: string) {
  try {
    localStorage.setItem(chave, valor);
  } catch {
    /* sem armazenamento: vale só o que está em memória */
  }
}

export function lerDoLocal(): Record<string, string> {
  const chaves: Record<string, string> = {};
  for (const chave of CHAVES_ESPELHADAS) {
    const valor = lerChave(chave);
    if (valor !== null) chaves[chave] = valor;
  }
  return chaves;
}

function carimboDoLocal(): number {
  const valor = Number(lerChave(CHAVE_DO_CARIMBO));
  return Number.isFinite(valor) && valor > 0 ? valor : 0;
}

export type ResultadoDaRestauracao = "restaurado" | "local" | "sem-copia";

/** Ao abrir, antes do React ler o localStorage. Nunca rejeita. */
export async function restaurarDoDisco(ler: Leitor = lerDoDisco, esperaMaxima = ESPERA_MAXIMA_MS): Promise<ResultadoDaRestauracao> {
  let disco: EstadoDaInterface | null = null;
  try {
    disco = await Promise.race([
      ler(),
      new Promise<null>((resolver) => setTimeout(() => resolver(null), esperaMaxima)),
    ]);
  } catch {
    disco = null;
  }
  const chaves = Object.entries(disco?.chaves ?? {}).filter(([chave]) => (CHAVES_ESPELHADAS as readonly string[]).includes(chave));
  if (!disco || chaves.length === 0) return "sem-copia";
  if (!(disco.salvo_em > carimboDoLocal())) return "local";
  for (const [chave, valor] of chaves) gravarChave(chave, valor);
  gravarChave(CHAVE_DO_CARIMBO, String(disco.salvo_em));
  return "restaurado";
}

let ultimoEspelhado = "";

/** Grava no disco o que mudou desde a última vez (nada, se nada mudou). */
export async function espelharAgora(gravar: Gravador = gravarNoDisco, agora = Date.now()): Promise<void> {
  const chaves = lerDoLocal();
  const assinatura = JSON.stringify(chaves);
  if (assinatura === ultimoEspelhado || Object.keys(chaves).length === 0) return;
  ultimoEspelhado = assinatura;
  const salvo_em = Math.max(agora, carimboDoLocal() + 1);
  gravarChave(CHAVE_DO_CARIMBO, String(salvo_em));
  try {
    await gravar({ salvo_em, chaves });
  } catch {
    // Tenta de novo na próxima volta.
    ultimoEspelhado = "";
  }
}

/** Só para testes. */
export function esquecerUltimoEspelhado() {
  ultimoEspelhado = "";
}

/** Espelha a cada 2 s e quando a janela some ou fecha. */
export function iniciarEspelho(gravar: Gravador = gravarNoDisco): () => void {
  const espelhar = () => void espelharAgora(gravar);
  const aoEsconder = () => { if (document.visibilityState === "hidden") espelhar(); };
  const relogio = window.setInterval(espelhar, INTERVALO_DO_ESPELHO_MS);
  document.addEventListener("visibilitychange", aoEsconder);
  window.addEventListener("pagehide", espelhar);
  window.addEventListener("beforeunload", espelhar);
  espelhar();
  return () => {
    window.clearInterval(relogio);
    document.removeEventListener("visibilitychange", aoEsconder);
    window.removeEventListener("pagehide", espelhar);
    window.removeEventListener("beforeunload", espelhar);
  };
}
