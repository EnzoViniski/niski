// Fork (Editor de vídeo): arrastar arquivos do Finder para uma área do editor. O Tauri entrega os
// caminhos (não o conteúdo): o Rust copia/clona direto, sem passar gigabytes pela interface.
// Mesma conta de posição do chat (`features/conversa/AbaDeAgente.tsx`).
import { useEffect, useRef, useState, type RefObject } from "react";

export function useSoltarArquivos(area: RefObject<HTMLElement | null>, aoSoltar: (caminhos: string[]) => void, ligado = true) {
  const [arrastando, setArrastando] = useState(false);
  const ultimo = useRef(aoSoltar);
  ultimo.current = aoSoltar;
  useEffect(() => {
    if (!ligado) return;
    let desligar: (() => void) | undefined;
    let desmontado = false;
    const dentro = (posicao: { x: number; y: number }) => {
      const caixa = area.current?.getBoundingClientRect();
      const escala = window.devicePixelRatio || 1;
      const x = posicao.x / escala, y = posicao.y / escala;
      return Boolean(caixa && caixa.width > 0 && x >= caixa.left && x <= caixa.right && y >= caixa.top && y <= caixa.bottom);
    };
    void (async () => {
      try {
        const { getCurrentWebviewWindow } = await import("@tauri-apps/api/webviewWindow");
        const parar = await getCurrentWebviewWindow().onDragDropEvent((evento) => {
          const dados = evento.payload;
          if (dados.type === "leave") { setArrastando(false); return; }
          const aqui = dentro(dados.position);
          if (dados.type === "enter" || dados.type === "over") { setArrastando(aqui); return; }
          setArrastando(false);
          if (aqui && dados.paths.length) ultimo.current(dados.paths);
        });
        if (desmontado) parar(); else desligar = parar;
      } catch {
        // Fora do Tauri (testes): só o botão "Escolher arquivos".
      }
    })();
    return () => { desmontado = true; desligar?.(); };
  }, [area, ligado]);
  return arrastando;
}

/** Diálogo nativo de arquivos (vários). Devolve os caminhos escolhidos. */
export async function escolherArquivos(): Promise<string[]> {
  const { open } = await import("@tauri-apps/plugin-dialog");
  const escolha = await open({ multiple: true, directory: false, title: "Escolha vídeos, fotos, áudio, fontes ou logos" });
  if (!escolha) return [];
  return Array.isArray(escolha) ? escolha : [escolha];
}

/** Uma pasta do computador (ex.: a pasta da Medza). Devolve o caminho real ou `null`. */
export async function escolherPasta(titulo = "Escolha a pasta do computador para este vídeo"): Promise<string | null> {
  const { open } = await import("@tauri-apps/plugin-dialog");
  const escolha = await open({ multiple: false, directory: true, title: titulo });
  const caminho = Array.isArray(escolha) ? escolha[0] : escolha;
  return caminho ?? null;
}
