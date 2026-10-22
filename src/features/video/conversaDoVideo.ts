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

// Fork (0.22): confere sempre que o editor está à vista, não só com o agente "trabalhando": pega a
// edição de qualquer agente (Codex, Claude no terminal, outro app) e a feita à mão no arquivo.
// Só `stat` em poucos arquivos; janela escondida não confere.
const INTERVALO_COM_AGENTE_MS = 1000;
const INTERVALO_PARADO_MS = 1500;

const ATIVOS: ReadonlySet<TerminalState> = new Set(["working", "approval_required"]);

export const lerAssinatura = (projeto: string) => invoke<string>("videos_assinatura", { id: projeto });

/**
 * `aoComecar`: o agente começou a trabalhar (guardar versão). `aoMudar`: arquivos do vídeo mudaram
 * no disco. `gravando(p)`: o editor está gravando ele mesmo (a mudança dele não conta como de fora).
 */
export function useAgenteNoProjeto(projeto: string, sessionId: string | null | undefined, { aoComecar, aoMudar }: {
  aoComecar: () => void;
  /** `preview`: mudou a composição (index, compositions/, assets/); senão só briefing/mídias. */
  aoMudar: (mudou: { preview: boolean }) => void;
}) {
  const estado = useSession(sessionId)?.state;
  const ativo = !!estado && ATIVOS.has(estado);
  const ativoRef = useRef(ativo);
  ativoRef.current = ativo;
  const base = useRef<string | null>(null);
  const pendentes = useRef(0);
  const chamadas = useRef({ aoComecar, aoMudar });
  chamadas.current = { aoComecar, aoMudar };

  const conferir = useCallback(async () => {
    if (pendentes.current > 0) return;
    const agora = await lerAssinatura(projeto).catch(() => null);
    if (agora === null || pendentes.current > 0) return;
    if (base.current !== null && agora !== base.current) {
      chamadas.current.aoMudar({ preview: agora.split(":")[0] !== base.current.split(":")[0] });
    }
    base.current = agora;
  }, [projeto]);

  const rebase = useCallback(async () => { base.current = await lerAssinatura(projeto).catch(() => base.current); }, [projeto]);

  /** Envolve uma gravação do próprio editor: enquanto ela roda, nada conta como mudança de fora. */
  const gravando = useCallback(<T,>(promessa: Promise<T>) => {
    pendentes.current++;
    return promessa.finally(async () => { await rebase(); pendentes.current--; });
  }, [rebase]);

  // Agente começou um pedido: versão antes (desfazer volta nela).
  useEffect(() => { if (ativo) chamadas.current.aoComecar(); }, [ativo]);

  useEffect(() => {
    let vivo = true;
    let timer = 0;
    void rebase();
    const laco = async () => {
      if (!document.hidden) await conferir();
      if (vivo) timer = window.setTimeout(() => void laco(), ativoRef.current ? INTERVALO_COM_AGENTE_MS : INTERVALO_PARADO_MS);
    };
    timer = window.setTimeout(() => void laco(), INTERVALO_PARADO_MS);
    return () => { vivo = false; window.clearTimeout(timer); };
  }, [conferir, rebase]);

  // O agente acabou: confere já (a última edição pode ter caído entre uma conferida e outra).
  useEffect(() => { if (!ativo) void conferir(); }, [ativo, conferir]);

  return { ativo, rebase, gravando };
}
