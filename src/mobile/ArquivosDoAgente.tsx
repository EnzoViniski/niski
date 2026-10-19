import { useState } from "react";
import { iconForPath } from "../features/files/fileIcons";
import { FolderIcon, LoaderIcon, SaveIcon } from "../components/ui/PixelIcon";
import { nomeDoCaminho } from "./anexos";
import { request } from "./api";

/**
 * Fork: os arquivos que o agente gerou (PDF, planilha, apresentação, página…) como cartões na
 * conversa, como nos apps do Claude e do Codex. O engine os acha no transcript (`entregaveis.rs`).
 *
 * No app do PC o toque abre no app padrão do Mac (comando `abrir_entregavel`, que só aceita
 * extensão de entregável); no celular o arquivo vem pelo engine e é baixado.
 */
const NO_PC = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

async function comandoDoPc(comando: string, caminho: string): Promise<void> {
  const { invoke } = await import("@tauri-apps/api/core");
  await invoke(comando, { caminho });
}

/** `/Users/eu/projeto/saida/relatorio.pdf` → `…/projeto/saida`. */
function pastaCurta(caminho: string): string {
  const partes = caminho.split(/[\\/]/).filter(Boolean);
  partes.pop();
  return partes.length > 2 ? `…/${partes.slice(-2).join("/")}` : `/${partes.join("/")}`;
}

function base64ParaBlob(dados: string, tipo: string): Blob {
  const binario = atob(dados);
  const bytes = new Uint8Array(binario.length);
  for (let i = 0; i < binario.length; i += 1) bytes[i] = binario.charCodeAt(i);
  return new Blob([bytes], { type: tipo });
}

async function baixar(conversa: string, n: number, k: number): Promise<void> {
  const { nome, tipo, dados } = await request<{ nome: string; tipo: string; dados: string }>(
    `/conversas/${encodeURIComponent(conversa)}/arquivos/${n}/${k}`);
  const url = URL.createObjectURL(base64ParaBlob(dados, tipo));
  const link = document.createElement("a");
  link.href = url;
  link.download = nome;
  document.body.appendChild(link);
  link.click();
  link.remove();
  window.setTimeout(() => URL.revokeObjectURL(url), 60_000);
}

function CartaoDeArquivo({ conversa, n, k, caminho }: { conversa: string; n: number; k: number; caminho: string }) {
  const [abrindo, setAbrindo] = useState(false);
  const [erro, setErro] = useState<string | null>(null);
  const nome = nomeDoCaminho(caminho);
  const Icone = iconForPath(nome);

  async function abrir() {
    setAbrindo(true); setErro(null);
    try {
      if (NO_PC) await comandoDoPc("abrir_entregavel", caminho);
      else await baixar(conversa, n, k);
    } catch (motivo) {
      setErro(motivo instanceof Error ? motivo.message : String(motivo));
    } finally {
      setAbrindo(false);
    }
  }

  return (
    <li className="arquivo-gerado">
      <button type="button" className="arquivo-gerado-abrir" onClick={() => void abrir()} disabled={abrindo}
        title={NO_PC ? `Abrir ${caminho}` : `Baixar ${nome}`}>
        <span className="arquivo-gerado-icone" aria-hidden>
          {abrindo ? <LoaderIcon className="anexo-girando h-5 w-5" /> : <Icone className="h-5 w-5" />}
        </span>
        <span className="arquivo-gerado-texto">
          <span className="arquivo-gerado-nome">{nome}</span>
          <span className="arquivo-gerado-pasta">{erro ?? pastaCurta(caminho)}</span>
        </span>
        {!NO_PC && <SaveIcon aria-hidden className="h-4 w-4 shrink-0 text-text-muted" />}
      </button>
      {NO_PC && (
        <button type="button" className="arquivo-gerado-pasta-botao" aria-label={`Mostrar ${nome} no Finder`} title="Mostrar no Finder"
          onClick={() => void comandoDoPc("mostrar_entregavel_na_pasta", caminho).catch((motivo) => setErro(String(motivo)))}>
          <FolderIcon aria-hidden className="h-4 w-4" />
        </button>
      )}
    </li>
  );
}

export interface ArquivosDaMensagem { conversa: string; n: number; caminhos: string[] }

export function ArquivosDoAgente({ conversa, n, caminhos }: ArquivosDaMensagem) {
  if (caminhos.length === 0) return null;
  return (
    <ul className="arquivos-gerados" aria-label="Arquivos gerados pelo agente">
      {caminhos.map((caminho, k) => <CartaoDeArquivo key={`${caminho}-${k}`} conversa={conversa} n={n} k={k} caminho={caminho} />)}
    </ul>
  );
}
