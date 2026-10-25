// Fork (Bíblia do dia): quando a leitura cobre a tela. Sozinha, uma vez por dia, logo depois dos
// exercícios da manhã; a qualquer hora pela lateral, pela paleta (⌘K), pela Rotina ou pelas Configurações.
import { useCallback, useEffect, useState } from "react";
import { diaLocal, estadoDaManha } from "../manha/rotina";
import { deveAbrirSozinha, estadoDaLeitura } from "./leitura";

let pedidoManual: (() => void) | null = null;

/** Abre a leitura agora, em qualquer horário (mesmo já lida: mostra o que leu e deixa ler mais). */
export function abrirLeituraAgora() { pedidoManual?.(); }

function abreSozinha(agora = new Date()) {
  const m = estadoDaManha();
  const s = m.sessao;
  return deveAbrirSozinha(estadoDaLeitura(), {
    ativo: m.ajustes.ativo && m.ajustes.temas.length > 0,
    horario: m.ajustes.horario,
    terminouHoje: !!s && s.dia === diaLocal(agora) && s.fim !== "",
  }, agora);
}

/** Depois de fechar o resumo da manhã: abre na hora, sem esperar a próxima conferida. */
export function abrirLeituraSeForAHora() { if (abreSozinha()) pedidoManual?.(); }

export function useLeituraDoDia(bloqueada: boolean) {
  const [aberta, setAberta] = useState(false);
  useEffect(() => {
    // Com o aquecimento na tela, espera: a leitura vem depois dele.
    const conferir = () => { if (!bloqueada && abreSozinha()) setAberta(true); };
    conferir();
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
  }, [bloqueada]);
  const fechar = useCallback(() => setAberta(false), []);
  return { aberta, fechar };
}
