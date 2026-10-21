// Fork (Editor de vídeo): a pasta do computador do vídeo (ex.: a pasta da Medza com identidade
// visual, logos e informações). O agente lê essa pasta; o vídeo continua salvo no app.
import { useState } from "react";
import { FolderIcon } from "../../components/ui/PixelIcon";
import * as disco from "./armazenamento";
import { escolherPasta } from "./soltarArquivos";

export function nomeDaPasta(caminho: string) { return caminho.replace(/[\\/]+$/, "").split(/[\\/]/).pop() || caminho; }

/** Abre o seletor do sistema e confere a pasta no Rust. `null` = desistiu. Erro vira exceção com texto. */
export async function escolherPastaDoComputador(): Promise<string | null> {
  const caminho = await escolherPasta();
  return caminho ? disco.conferirPasta(caminho) : null;
}

export function PastaDoComputador({ caminho, desligado, onMudar }: {
  caminho: string | null; desligado?: boolean; onMudar: (caminho: string | null) => Promise<boolean> | void;
}) {
  const [erro, setErro] = useState<string | null>(null);
  const trocar = async () => {
    setErro(null);
    try { const c = await escolherPastaDoComputador(); if (c) await onMudar(c); }
    catch (e) { setErro(e instanceof Error ? e.message : String(e)); }
  };
  return (
    <span className="video-pasta-pc" title={erro ?? (caminho ? `${caminho}\nO agente lê esta pasta. Clique para trocar.` : "Escolha uma pasta do computador (ex.: a da sua marca) para o agente usar neste vídeo")}>
      <button type="button" className={`video-pasta-pc-botao${caminho ? " is-ligada" : ""}${erro ? " is-erro" : ""}`} disabled={desligado} onClick={() => void trocar()}>
        <FolderIcon size={13} aria-hidden="true" />
        <span>{caminho ? nomeDaPasta(caminho) : "Pasta do computador…"}</span>
      </button>
      {caminho && (
        <button type="button" className="video-mini-botao" disabled={desligado} aria-label="Tirar a pasta do computador deste vídeo"
          title="Tirar a pasta deste vídeo (nada é apagado)" onClick={() => void onMudar(null)}>×</button>
      )}
    </span>
  );
}
