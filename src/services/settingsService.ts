import type { AppSettings } from "../types/settings";
import { DEFAULT_SETTINGS } from "../types/settings";

const STORAGE_KEY = "omni-agents-settings";
const LEGACY_KEY = "pdf-to-markdown-settings";

export function loadSettings(): AppSettings {
  try {
    const raw =
      localStorage.getItem(STORAGE_KEY) ?? localStorage.getItem(LEGACY_KEY);
    return mesclarComPadrao(raw ? JSON.parse(raw) : {});
  } catch {
    return { ...DEFAULT_SETTINGS };
  }
}

/** Campo a campo, como o workspace (`consertarWorkspace.ts`). Um valor salvo com tipo errado
 *  (`avisos: null`, `shortcuts: []`, número em texto) cai no padrão sozinho em vez de derrubar a tela
 *  ou o arquivo inteiro; chave que esta versão não conhece (de uma mais nova) é preservada. */
function tipoDe(valor: unknown): string {
  return valor === null ? "null" : Array.isArray(valor) ? "array" : typeof valor;
}

export function mesclarComPadrao(salvo: unknown): AppSettings {
  if (tipoDe(salvo) !== "object") return { ...DEFAULT_SETTINGS };
  const padrao = DEFAULT_SETTINGS as unknown as Record<string, unknown>;
  const resultado: Record<string, unknown> = { ...padrao };
  for (const [chave, valor] of Object.entries(salvo as Record<string, unknown>)) {
    if (!(chave in padrao) || tipoDe(valor) === tipoDe(padrao[chave])) resultado[chave] = valor;
  }
  return resultado as unknown as AppSettings;
}

export function saveSettings(settings: AppSettings): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(settings));
  } catch {
    // Silently fail — settings are not critical
  }
}

/** Bytes ocupados pelas chaves do app no localStorage (aproximado). */
export function storageUsedBytes(): number {
  try {
    let total = 0;
    for (let i = 0; i < localStorage.length; i++) {
      const key = localStorage.key(i);
      if (!key) continue;
      total += key.length + (localStorage.getItem(key)?.length ?? 0);
    }
    return total * 2; // UTF-16
  } catch {
    return 0;
  }
}
