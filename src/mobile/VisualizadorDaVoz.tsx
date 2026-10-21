import { useEffect, useState } from "react";

/*
 * Fork: barrinhas do volume da voz ao lado do microfone, enquanto grava (pedido do Enzo,
 * 20/10/2026). O Rust devolve o nível dos últimos trechos de 50 ms (`voz_niveis`, em
 * `src-tauri/src/voz.rs`); perguntando a cada ~90 ms, as barras correm da direita para a esquerda.
 * Silêncio vira um pontinho, fala vira barra.
 */

const BARRAS = 18;
const INTERVALO_MS = 90;
/** Abaixo disso a barra é desenhada como ponto (silêncio de sala, respiração). */
const LIMIAR_DO_PONTO = 0.12;

async function lerNiveis(): Promise<number[]> {
  const { invoke } = await import("@tauri-apps/api/core");
  const niveis = await invoke<unknown>("voz_niveis", { barras: BARRAS });
  return Array.isArray(niveis) ? niveis.map((n) => (typeof n === "number" ? n : 0)) : [];
}

export function VisualizadorDaVoz({ ativo }: { ativo: boolean }) {
  const [niveis, setNiveis] = useState<number[]>(() => Array(BARRAS).fill(0));

  useEffect(() => {
    if (!ativo) { setNiveis(Array(BARRAS).fill(0)); return; }
    let vivo = true;
    let lendo = false;
    const id = window.setInterval(() => {
      // Uma leitura por vez: se a anterior atrasou, pula esta em vez de empilhar chamadas.
      if (lendo) return;
      lendo = true;
      void lerNiveis()
        .then((lidos) => { if (vivo && lidos.length) setNiveis(lidos); })
        .catch(() => undefined) // visualizador é enfeite: falha aqui não atrapalha a gravação
        .finally(() => { lendo = false; });
    }, INTERVALO_MS);
    return () => { vivo = false; window.clearInterval(id); };
  }, [ativo]);

  return (
    <span className="voz-onda" aria-hidden data-ativo={ativo || undefined}>
      {niveis.map((nivel, i) => (
        <span key={i} className={nivel < LIMIAR_DO_PONTO ? "voz-onda-ponto" : "voz-onda-barra"}
          style={nivel < LIMIAR_DO_PONTO ? undefined : { height: `${Math.round(25 + nivel * 75)}%` }} />
      ))}
    </span>
  );
}
