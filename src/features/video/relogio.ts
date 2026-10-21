// Fork (Editor de vídeo): tempo do preview num lugar só. A agulha da timeline e o mostrador assinam
// aqui; o resto do editor não re-renderiza a cada quadro.
import { useSyncExternalStore } from "react";

export interface Relogio { tempo: number; duracao: number; tocando: boolean; pronto: boolean }

export function criarRelogio() {
  let valor: Relogio = { tempo: 0, duracao: 0, tocando: false, pronto: false };
  const ouvintes = new Set<() => void>();
  return {
    ler: () => valor,
    mudar(parcial: Partial<Relogio>) {
      const novo = { ...valor, ...parcial };
      if (novo.tempo === valor.tempo && novo.duracao === valor.duracao && novo.tocando === valor.tocando && novo.pronto === valor.pronto) return;
      valor = novo;
      for (const o of ouvintes) o();
    },
    assinar(o: () => void) { ouvintes.add(o); return () => { ouvintes.delete(o); }; },
  };
}

export type RelogioDoPreview = ReturnType<typeof criarRelogio>;

export function useRelogio(relogio: RelogioDoPreview) {
  return useSyncExternalStore(relogio.assinar, relogio.ler, relogio.ler);
}

/** `01:05.20` (minutos, segundos e centésimos). */
export function formatarTempo(t: number) {
  const s = Math.max(0, t);
  const m = Math.floor(s / 60);
  const resto = s - m * 60;
  return `${String(m).padStart(2, "0")}:${resto.toFixed(2).padStart(5, "0")}`;
}
