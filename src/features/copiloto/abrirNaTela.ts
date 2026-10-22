// Fork (Copiloto abre conversa na tela, 2026-10-22): conversa que o copiloto criou com `na_tela`
// ("abre uma conversa na tela com o Codex…") vira uma aba do workspace atual, uma vez.
import { useEffect, useRef } from "react";
import { listTerminalSessions } from "../terminal/terminalService";
import { pedirAbrirConversa } from "../orquestracao/abrirFluxo";
import type { NoAoVivo } from "../orquestracao/orquestracaoService";

/** Só as recém-criadas: reabrir o app (ou a janela do copiloto) não rouba o foco com conversa antiga. */
export const PRAZO_PARA_ABRIR_NA_TELA_MS = 2 * 60_000;

export function paraAbrirNaTela(agentes: NoAoVivo[], vistos: ReadonlySet<string>, agora = Date.now()): NoAoVivo[] {
  return agentes.filter((a) => a.na_tela && !vistos.has(a.id) && a.criado_em != null && agora - a.criado_em < PRAZO_PARA_ABRIR_NA_TELA_MS);
}

/** Abre numa aba (`ATTACH_TERMINAL` no projeto ativo) cada conversa nova `na_tela`. A sessão pode ainda
 * não estar na lista: tenta de novo na próxima leitura do fluxo. */
export function useAbrirNaTela(agentes: NoAoVivo[]) {
  const vistos = useRef(new Set<string>());
  useEffect(() => {
    if (!paraAbrirNaTela(agentes, vistos.current).length) return;
    let vivo = true;
    void listTerminalSessions().then((sessoes) => {
      if (!vivo) return;
      for (const agente of paraAbrirNaTela(agentes, vistos.current)) {
        const sessao = sessoes.find((s) => s.conversation_id === agente.id);
        if (!sessao) continue;
        vistos.current.add(agente.id);
        pedirAbrirConversa(sessao.id, agente.titulo ?? sessao.name ?? "Conversa do copiloto");
      }
    }).catch(() => undefined);
    return () => { vivo = false; };
  }, [agentes]);
}
