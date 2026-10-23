import { createContext, useCallback, useContext, useEffect, useState } from "react";
import { SaveIcon, FolderIcon, PlayIcon, ReloadIcon } from "../components/ui/PixelIcon";
import { nomeDoCaminho } from "./anexos";
import { request } from "./api";
import "./videoDoAgente.css";

/**
 * Fork (vídeo na conversa): o vídeo que o agente gerou (HyperFrames, ffmpeg…) toca dentro da conversa.
 *
 * - No app do PC, direto do disco (`convertFileSrc`, protocolo `asset:` já liberado no CSP para mídia).
 * - No celular, pelo engine: um bilhete curto (`POST /conversas/{id}/videos/{n}/{k}`) vira o endereço do
 *   `<video>` (`/video/<bilhete>`), servido em pedaços — dá para pular sem baixar tudo.
 * O mesmo vídeo citado em mais de uma mensagem (a saída do render e a resposta final) toca só na última;
 * as outras mostram o nome (`UltimoLugarDoVideo`).
 */
const NO_PC = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

export const EXTENSOES_DE_VIDEO = [".mp4", ".mov", ".webm"];
export const ehVideo = (caminho: string) => EXTENSOES_DE_VIDEO.some((e) => caminho.toLowerCase().endsWith(e));

/** Caminho do vídeo → número da última mensagem que o mostra. Sem provedor, todo vídeo toca. */
export const UltimoLugarDoVideo = createContext<Map<string, number> | null>(null);

/** O mapa a partir das mensagens carregadas (`n` = número da mensagem na conversa). */
export function ultimosLugares(mensagens: { n: number; arquivos?: string[] }[]): Map<string, number> {
  const mapa = new Map<string, number>();
  for (const m of mensagens) for (const caminho of m.arquivos ?? []) if (ehVideo(caminho)) mapa.set(caminho, m.n);
  return mapa;
}

async function enderecoNoPc(caminho: string): Promise<string> {
  const { convertFileSrc } = await import("@tauri-apps/api/core");
  return convertFileSrc(caminho);
}

async function enderecoNoCelular(conversa: string, n: number, k: number): Promise<string> {
  const r = await request<{ url: string }>(`/conversas/${encodeURIComponent(conversa)}/videos/${n}/${k}`, { method: "POST" });
  return r.url;
}

async function compartilhar(url: string, nome: string) {
  const blob = await (await fetch(url)).blob();
  const arquivo = new File([blob], nome, { type: blob.type || "video/mp4" });
  if (navigator.canShare?.({ files: [arquivo] })) { await navigator.share({ files: [arquivo], title: nome }); return; }
  const link = document.createElement("a");
  link.href = URL.createObjectURL(arquivo);
  link.download = nome;
  document.body.appendChild(link);
  link.click();
  link.remove();
  window.setTimeout(() => URL.revokeObjectURL(link.href), 60_000);
}

export function VideoDoAgente({ conversa, n, k, caminho }: { conversa: string; n: number; k: number; caminho: string }) {
  const ultimos = useContext(UltimoLugarDoVideo);
  const nome = nomeDoCaminho(caminho);
  const repetido = ultimos !== null && ultimos.has(caminho) && ultimos.get(caminho) !== n;
  const [url, setUrl] = useState<string | null>(null);
  const [erro, setErro] = useState<string | null>(null);
  const [tentativa, setTentativa] = useState(0);
  const [ocupado, setOcupado] = useState(false);

  useEffect(() => {
    if (repetido) return;
    let vivo = true;
    setErro(null); setUrl(null);
    (NO_PC ? enderecoNoPc(caminho) : enderecoNoCelular(conversa, n, k))
      .then((endereco) => { if (vivo) setUrl(endereco); })
      .catch(() => { if (vivo) setErro("O vídeo ainda não está pronto ou saiu da pasta."); });
    return () => { vivo = false; };
  }, [conversa, n, k, caminho, repetido, tentativa]);

  const deNovo = useCallback(() => setTentativa((t) => t + 1), []);

  if (repetido) {
    return <li className="video-agente is-repetido"><PlayIcon aria-hidden className="h-3.5 w-3.5" /><span>{nome}</span><span className="video-agente-dica">toca mais abaixo</span></li>;
  }
  return (
    <li className="video-agente">
      {url && !erro
        ? <video className="video-agente-player" src={url} controls playsInline preload="metadata" aria-label={`Vídeo ${nome}`}
            onError={() => setErro("O vídeo ainda não está pronto ou saiu da pasta.")} />
        : <div className="video-agente-vazio" role={erro ? "alert" : "status"}>{erro ?? "Preparando o vídeo…"}</div>}
      <div className="video-agente-barra">
        <span className="video-agente-nome" title={caminho}>{nome}</span>
        {erro && <button type="button" className="video-agente-botao" onClick={deNovo} aria-label="Tentar de novo" title="Tentar de novo"><ReloadIcon aria-hidden className="h-3.5 w-3.5" /></button>}
        {NO_PC && (
          <button type="button" className="video-agente-botao" aria-label={`Mostrar ${nome} no Finder`} title="Mostrar no Finder"
            onClick={() => void import("@tauri-apps/api/core").then(({ invoke }) => invoke("mostrar_entregavel_na_pasta", { caminho })).catch(() => undefined)}>
            <FolderIcon aria-hidden className="h-3.5 w-3.5" />
          </button>
        )}
        {!NO_PC && url && !erro && (
          <button type="button" className="video-agente-botao" disabled={ocupado} aria-label={`Salvar ou compartilhar ${nome}`} title="Salvar ou compartilhar"
            onClick={() => { setOcupado(true); void compartilhar(url, nome).catch(() => undefined).finally(() => setOcupado(false)); }}>
            <SaveIcon aria-hidden className="h-3.5 w-3.5" />
          </button>
        )}
      </div>
    </li>
  );
}
