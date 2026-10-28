import { useEffect } from "react";
import { definirPausaDeEnergia, INATIVIDADE_MS } from "../lib/energia";

/**
 * Liga a pausa de energia (`src/lib/energia.ts`) com a janela oculta, sem foco ou parada há
 * `INATIVIDADE_MS`. Qualquer mouse, tecla, rolagem ou foco de volta retoma na hora.
 */
export function useEconomiaDeEnergia() {
  useEffect(() => {
    let ultimaAtividade = Date.now();
    let timer: number | undefined;

    const avaliar = () => {
      window.clearTimeout(timer);
      const foco = typeof document.hasFocus === "function" ? document.hasFocus() : true;
      const parado = Date.now() - ultimaAtividade >= INATIVIDADE_MS;
      definirPausaDeEnergia(document.hidden || !foco || parado);
      // Só reconfere a inatividade enquanto está visível e em foco: o resto chega por evento.
      if (!document.hidden && foco && !parado) {
        timer = window.setTimeout(avaliar, INATIVIDADE_MS - (Date.now() - ultimaAtividade) + 50);
      }
    };
    // pointermove dispara muito: só reavalia quando estava pausado ou o timer precisa recomeçar.
    let ultimaReavaliacao = 0;
    const atividade = () => {
      ultimaAtividade = Date.now();
      if (ultimaAtividade - ultimaReavaliacao < 1_000) return;
      ultimaReavaliacao = ultimaAtividade;
      avaliar();
    };
    const eventos = ["pointermove", "pointerdown", "keydown", "wheel", "touchstart"] as const;
    for (const nome of eventos) window.addEventListener(nome, atividade, { passive: true, capture: true });
    const aoMudarFoco = () => { ultimaAtividade = Date.now(); avaliar(); };
    // No `blur` o `hasFocus()` ainda pode dizer true: pausa direto.
    const aoPerderFoco = () => { window.clearTimeout(timer); definirPausaDeEnergia(true); };
    window.addEventListener("focus", aoMudarFoco);
    window.addEventListener("blur", aoPerderFoco);
    document.addEventListener("visibilitychange", aoMudarFoco);
    avaliar();
    return () => {
      window.clearTimeout(timer);
      for (const nome of eventos) window.removeEventListener(nome, atividade, { capture: true });
      window.removeEventListener("focus", aoMudarFoco);
      window.removeEventListener("blur", aoPerderFoco);
      document.removeEventListener("visibilitychange", aoMudarFoco);
      definirPausaDeEnergia(false);
    };
  }, []);
}
