import { useEffect, useState } from "react";
import { ensureEngine, listTerminalSessions, type TerminalSession } from "./terminalService";

export function useTerminalSessions() {
  const [sessions, setSessions] = useState<TerminalSession[]>([]);
  const [online, setOnline] = useState(false);

  useEffect(() => {
    let cancelled = false;
    let timer: number | undefined;
    let ultimaAssinatura = "";
    // Fork (energia): `ensureEngine` (ping + conferência do binário) só na primeira leitura e
    // depois de uma falha; com o engine respondendo, a listagem já prova que ele está vivo.
    let conferirEngine = true;

    async function refresh() {
      try {
        if (conferirEngine) await ensureEngine();
        const next = await listTerminalSessions();
        conferirEngine = false;
        if (!cancelled) {
          // Fork: roda a cada 1s e o App inteiro (lateral, workspace, fundo) depende disto. Lista
          // igual à anterior = mesma referência, para o React nem re-renderizar com tudo parado.
          // Fork (energia): `output_seq` sobe a cada pedaço de saída e `last_activity_at_ms` a cada
          // redesenho da TUI — com um agente transmitindo, o App inteiro re-renderizava a cada 1 s.
          // Ninguém do App usa `output_seq` (o terminal lê o seu pelo snapshot); a lateral mostra
          // "há N min", então a atividade entra arredondada ao minuto.
          const assinatura = JSON.stringify(next, (chave, valor) =>
            chave === "output_seq" ? undefined
            : chave === "last_activity_at_ms" && typeof valor === "number" ? Math.floor(valor / 60_000)
            : valor);
          if (assinatura !== ultimaAssinatura) {
            ultimaAssinatura = assinatura;
            setSessions(next);
          }
          setOnline(true);
        }
      } catch {
        conferirEngine = true;
        if (!cancelled) setOnline(false);
      }
      // Fork (energia): janela oculta (minimizada, outro Space) lê a cada 5 s; ao voltar, lê na hora.
      if (!cancelled) timer = window.setTimeout(refresh, document.hidden ? 5_000 : 1_000);
    }

    const aoVoltar = () => {
      if (cancelled || document.hidden) return;
      window.clearTimeout(timer);
      void refresh();
    };
    document.addEventListener("visibilitychange", aoVoltar);
    void refresh();
    return () => {
      cancelled = true;
      if (timer) window.clearTimeout(timer);
      document.removeEventListener("visibilitychange", aoVoltar);
    };
  }, []);

  return { sessions, online };
}
