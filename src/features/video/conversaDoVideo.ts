// Fork (Editor de vídeo): a conversa normal do app ligada a um projeto de vídeo.
//
// - O engine conhece a conversa pelo projeto `video:<id>`; quem lista "projetos soltos" (recuperar
//   workspace) pula esses ids.
// - Enquanto o agente trabalha, o editor confere a cada segundo se os arquivos do vídeo mudaram
//   (`videos_assinatura`, só data e tamanho) e recarrega preview e timeline. Parado, não confere nada.
// - Quando o agente começa um pedido, o app guarda antes uma versão (desfazer volta nela).
import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useRef } from "react";
import { useSession } from "../terminal/sessionProviders";
import type { TerminalState } from "../terminal/terminalService";

export { ehConversaDoVideo, idDoProjetoDoVideo } from "./projetoDoVideo";

const INTERVALO_MS = 1000;

const ATIVOS: ReadonlySet<TerminalState> = new Set(["working", "approval_required"]);

export const lerAssinatura = (projeto: string) => invoke<string>("videos_assinatura", { id: projeto });

/**
 * `aoComecar`: o agente começou a trabalhar (guardar versão). `aoMudar`: arquivos do vídeo mudaram
 * no disco. Devolve `rebase`, para o editor marcar como vista uma mudança que ele mesmo gravou.
 */
export function useAgenteNoProjeto(projeto: string, sessionId: string | null | undefined, { aoComecar, aoMudar }: {
  aoComecar: () => void; aoMudar: () => void;
}) {
  const estado = useSession(sessionId)?.state;
  const ativo = !!estado && ATIVOS.has(estado);
  const base = useRef<string | null>(null);
  const chamadas = useRef({ aoComecar, aoMudar });
  chamadas.current = { aoComecar, aoMudar };

  const conferir = useCallback(async () => {
    const agora = await lerAssinatura(projeto).catch(() => null);
    if (agora === null) return;
    if (base.current !== null && agora !== base.current) chamadas.current.aoMudar();
    base.current = agora;
  }, [projeto]);

  const rebase = useCallback(() => { void lerAssinatura(projeto).then((a) => { base.current = a; }).catch(() => undefined); }, [projeto]);

  // Base ao abrir o editor: uma edição rápida antes da primeira conferida também conta.
  useEffect(() => { rebase(); }, [rebase]);

  useEffect(() => {
    if (!ativo) return;
    chamadas.current.aoComecar();
    let vivo = true;
    let timer = 0;
    const laco = async () => {
      await conferir();
      if (vivo) timer = window.setTimeout(() => void laco(), INTERVALO_MS);
    };
    void laco();
    return () => {
      vivo = false;
      window.clearTimeout(timer);
      // Última conferida: a edição final pode ter caído entre um segundo e outro.
      void conferir();
    };
  }, [ativo, conferir]);

  return { ativo, rebase };
}
