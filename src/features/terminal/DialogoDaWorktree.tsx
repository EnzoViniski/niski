import { useEffect, useState } from "react";
import { createPortal } from "react-dom";
import { Button } from "../../components/ui";
import { GitBranchIcon } from "../../components/ui/PixelIcon";
import type { TerminalSession } from "./terminalService";
import { estadoDaWorktreeIsolada, removerWorktreeIsolada, worktreeDaPasta, type EstadoDaWorktree } from "./worktreeIsolada";

/**
 * Ao encerrar e remover um agente que rodava numa worktree isolada, pergunta o que fazer com
 * ela. Só pergunta quando nenhuma outra sessão viva usa a mesma worktree. Devolve a pasta da sessão.
 */
export function worktreeParaPerguntar(sessions: TerminalSession[], sessionId: string): string | null {
  const sessao = sessions.find((item) => item.id === sessionId);
  const worktree = worktreeDaPasta(sessao?.cwd);
  if (!sessao || !worktree) return null;
  const outraUsa = sessions.some((item) => item.id !== sessionId && item.state !== "stopped" && item.state !== "crashed"
    && worktreeDaPasta(item.cwd)?.raiz === worktree.raiz);
  return outraUsa ? null : sessao.cwd;
}

/** Manter ou apagar a worktree. Com mudanças não commitadas, avisa quantas e quais antes de apagar. */
export function DialogoDaWorktree({ caminho, onFechar }: { caminho: string; onFechar: () => void }) {
  const [estado, setEstado] = useState<EstadoDaWorktree | null>(null);
  const [apagando, setApagando] = useState(false);
  const [erro, setErro] = useState<string | null>(null);

  useEffect(() => {
    let vivo = true;
    estadoDaWorktreeIsolada(caminho)
      .then((lido) => { if (!vivo) return; if (lido) setEstado(lido); else onFechar(); })
      .catch(() => { if (vivo) onFechar(); });
    return () => { vivo = false; };
  }, [caminho, onFechar]);

  useEffect(() => {
    const tecla = (event: KeyboardEvent) => { if (event.key === "Escape") onFechar(); };
    window.addEventListener("keydown", tecla);
    return () => window.removeEventListener("keydown", tecla);
  }, [onFechar]);

  if (!estado) return null;
  const sujo = estado.mudancas > 0;

  async function apagar() {
    setApagando(true);
    setErro(null);
    try {
      await removerWorktreeIsolada(estado!.caminho, sujo);
      onFechar();
    } catch (reason) {
      setErro(reason instanceof Error ? reason.message : String(reason));
      setApagando(false);
    }
  }

  return createPortal(
    <div className="fixed inset-0 z-50 grid place-items-center bg-black/40 p-4" onPointerDown={(e) => { if (e.target === e.currentTarget) onFechar(); }}>
      <div role="dialog" aria-modal="true" aria-labelledby="dialogo-worktree-titulo" className="popover w-full max-w-md p-4">
        <h2 id="dialogo-worktree-titulo" className="flex items-center gap-2 text-sm font-semibold text-text-primary">
          <GitBranchIcon className="h-4 w-4 text-accent" aria-hidden />
          Worktree do agente
        </h2>
        <p className="mt-2 text-xs text-text-secondary">
          O agente rodava numa worktree isolada na branch <strong className="text-text-primary">{estado.branch || "(sem branch)"}</strong>.
        </p>
        <p className="mt-1 break-all text-[11px] text-text-muted">{estado.caminho}</p>
        {sujo ? (
          <div role="alert" className="mt-3 border-l-2 border-danger bg-danger/10 px-3 py-2 text-xs text-danger">
            {estado.mudancas === 1 ? "1 mudança não commitada" : `${estado.mudancas} mudanças não commitadas`}: apagar a worktree perde
            {estado.mudancas === 1 ? " essa mudança" : " essas mudanças"}.
            <ul className="mt-1 list-disc pl-4 text-[11px]">
              {estado.exemplos.map((arquivo) => <li key={arquivo} className="break-all">{arquivo}</li>)}
              {estado.mudancas > estado.exemplos.length && <li>e mais {estado.mudancas - estado.exemplos.length}</li>}
            </ul>
          </div>
        ) : (
          <p className="mt-3 text-xs text-text-secondary">Sem mudanças pendentes: tudo o que o agente fez está commitado na branch.</p>
        )}
        <p className="mt-3 text-[11px] text-text-muted">Apagar remove só a pasta. A branch {estado.branch} continua no repositório.</p>
        {erro && <p role="alert" className="mt-2 text-xs text-danger">{erro}</p>}
        <div className="mt-4 flex justify-end gap-2">
          <Button variant="ghost" size="sm" onClick={onFechar} disabled={apagando}>Manter worktree</Button>
          <Button variant="danger" size="sm" onClick={() => void apagar()} loading={apagando}>
            {sujo ? "Apagar mesmo assim" : "Apagar worktree"}
          </Button>
        </div>
      </div>
    </div>,
    document.body
  );
}
