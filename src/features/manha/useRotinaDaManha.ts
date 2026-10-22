// Fork (Manhã): decide quando a rotina cobre a tela. Confere a cada 30 s e ao voltar para a janela
// (PC acordou do sono depois das 6h, app aberto agora). Depois de aberta, só fecha pelo usuário.
import { useCallback, useEffect, useState } from "react";
import { deveMostrar, estadoDaManha, refazerHoje, diaLocal } from "./rotina";

let pedidoManual: (() => void) | null = null;
/** ⌘K e Configurações: abre a sessão de hoje agora, mesmo antes do horário ou já feita. */
export function abrirRotinaAgora() {
  const s = estadoDaManha().sessao;
  if (s && s.dia === diaLocal(new Date()) && (s.fim || s.pulada || !s.questoes.length)) refazerHoje();
  pedidoManual?.();
}

export function useRotinaDaManha() {
  const [aberta, setAberta] = useState(() => deveMostrar(estadoDaManha(), new Date()));
  useEffect(() => {
    const conferir = () => { if (deveMostrar(estadoDaManha(), new Date())) setAberta(true); };
    const timer = setInterval(conferir, 30_000);
    window.addEventListener("focus", conferir);
    document.addEventListener("visibilitychange", conferir);
    pedidoManual = () => setAberta(true);
    return () => {
      clearInterval(timer);
      window.removeEventListener("focus", conferir);
      document.removeEventListener("visibilitychange", conferir);
      pedidoManual = null;
    };
  }, []);
  const fechar = useCallback(() => setAberta(false), []);
  return { aberta, fechar };
}
