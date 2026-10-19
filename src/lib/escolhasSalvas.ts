/**
 * Escolhas que o usuário faz uma vez e espera reencontrar ao reabrir o app: o último agente do
 * "Novo agente", a conta usada em cada agente e o modelo/esforço aplicados por último em cada um.
 * localStorage (por bundle id, como o resto da interface); qualquer falha vira "sem escolha".
 */

const CHAVE = "medza-escolhas";

export interface AjustesSalvos { model: string | null; effort: string | null }

interface Escolhas {
  agente?: string;
  contas?: Record<string, string>;
  ajustes?: Record<string, AjustesSalvos>;
}

function ler(): Escolhas {
  try {
    const valor = JSON.parse(localStorage.getItem(CHAVE) ?? "{}");
    return valor && typeof valor === "object" ? valor : {};
  } catch {
    return {};
  }
}

function gravar(mudar: (atual: Escolhas) => Escolhas) {
  try { localStorage.setItem(CHAVE, JSON.stringify(mudar(ler()))); } catch { /* sem storage: só não lembra */ }
}

export const ultimoAgente = () => ler().agente ?? null;
export const lembrarAgente = (agente: string) => gravar((e) => ({ ...e, agente }));

export const contaDoAgente = (provider: string) => ler().contas?.[provider] ?? null;
export const lembrarConta = (provider: string, perfil: string) =>
  gravar((e) => ({ ...e, contas: { ...e.contas, [provider]: perfil } }));

export const ajustesDoAgente = (provider: string) => ler().ajustes?.[provider] ?? null;
export const lembrarAjustes = (provider: string, ajustes: AjustesSalvos) =>
  gravar((e) => ({ ...e, ajustes: { ...e.ajustes, [provider]: ajustes } }));

/** Opções de modelo/esforço na sintaxe de cada CLI (as mesmas do `comando_de_retomada` do engine). */
export function opcoesDeModelo(provider: string, ajustes: AjustesSalvos | null): string {
  if (!ajustes) return "";
  const { model, effort } = ajustes;
  // Só valores simples: o texto vai direto para o shell da PTY.
  const seguro = (valor: string | null) => (valor && /^[\w.:-]+$/.test(valor) ? valor : null);
  const m = seguro(model);
  const e = seguro(effort);
  switch (provider) {
    case "claude":
    case "antigravity":
      return `${m ? ` --model ${m}` : ""}${e ? ` --effort ${e}` : ""}`;
    case "codex":
      return `${m ? ` -m ${m}` : ""}${e ? ` -c model_reasoning_effort=${e}` : ""}`;
    default:
      return "";
  }
}
