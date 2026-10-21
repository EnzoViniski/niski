/**
 * Fork (Roadmaps): modo foco. Ligado, só avisam os agentes que precisam de você de verdade (pedido de
 * aprovação ou pergunta, erro, limite de uso), e esses avisam mesmo com a janela em foco, porque você
 * não está olhando as conversas. O resto (terminou, subagente, agendamento) fica para depois.
 *
 * Só na memória: vale enquanto a tela de Roadmaps está aberta (o App desliga ao sair dela).
 */
import { useSyncExternalStore } from "react";
import type { PreferenciasDeAvisos, TipoDeAtividade } from "../features/atividade/tipos";

export const URGENTES: readonly TipoDeAtividade[] = ["precisa_de_voce", "erro", "limite"];

let ligadoDesde: number | null = null;
const ouvintes = new Set<() => void>();

function avisar() { for (const ouvinte of ouvintes) ouvinte(); }

export function modoFocoLigado(): boolean { return ligadoDesde !== null; }
export function focoDesde(): number | null { return ligadoDesde; }

export function definirModoFoco(ligado: boolean, agora = Date.now()) {
  if (ligado === modoFocoLigado()) return;
  ligadoDesde = ligado ? agora : null;
  avisar();
}

const inscrever = (ouvinte: () => void) => { ouvintes.add(ouvinte); return () => { ouvintes.delete(ouvinte); }; };

export function useModoFoco(): boolean {
  return useSyncExternalStore(inscrever, modoFocoLigado, modoFocoLigado);
}

/** Estudo de uma etapa em tela cheia: o App esconde a barra lateral enquanto vale. */
let telaCheia = false;
export function estudoEmTelaCheia(): boolean { return telaCheia; }
export function definirEstudoEmTelaCheia(valor: boolean) {
  if (valor === telaCheia) return;
  telaCheia = valor;
  avisar();
}
export function useEstudoEmTelaCheia(): boolean {
  return useSyncExternalStore(inscrever, estudoEmTelaCheia, estudoEmTelaCheia);
}

/** Preferências de aviso com o foco aplicado: urgente vira "sempre" (mantém o som), o resto "nunca". */
export function preferenciasNoFoco(preferencias: PreferenciasDeAvisos): PreferenciasDeAvisos {
  if (!modoFocoLigado()) return preferencias;
  return Object.fromEntries(Object.entries(preferencias).map(([tipo, p]) => [
    tipo, URGENTES.includes(tipo as TipoDeAtividade) ? { ...p, quando: "sempre" } : { ...p, quando: "nunca" },
  ])) as PreferenciasDeAvisos;
}
