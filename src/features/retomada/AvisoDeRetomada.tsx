import { useEffect, useState } from "react";
import { Button } from "../../components/ui";
import {
  continuarEmConversaNova,
  prepararRetomada,
  reabrirEmOutraPasta,
  semCodigo,
} from "./retomadaService";

/*
 * Fork (retomar-sempre): saídas na própria aba quando ela não consegue voltar.
 * - Pasta sumida (worktree apagada): "Abrir no repositório principal" (sugestão do engine) ou no
 *   projeto da aba; a mesma sessão reabre lá com `--resume`.
 * - A CLI recusou o `--resume` ("No conversation found"): conversa nova com as últimas mensagens da
 *   antiga como contexto, nesta mesma aba.
 */

interface SessaoDaAba {
  sessionId: string;
  projectId: string;
  projectPath: string;
  cwd: string;
  provider?: string;
  profileId?: string;
  externalSessionId?: string;
  titulo?: string;
}

export function PastaSumida({ erro, sessao, onReaberta }: { erro: string; sessao: SessaoDaAba; onReaberta: () => void }) {
  const [principal, setPrincipal] = useState<string | null>(null);
  const [falha, setFalha] = useState<string | null>(null);
  const [ocupado, setOcupado] = useState(false);

  useEffect(() => {
    let vivo = true;
    void prepararRetomada({ provider: sessao.provider ?? "terminal", sessionId: sessao.externalSessionId ?? sessao.sessionId, cwd: sessao.cwd })
      .then((diagnostico) => { if (vivo) setPrincipal(diagnostico?.pasta_principal ?? null); })
      .catch(() => undefined);
    return () => { vivo = false; };
  }, [sessao.provider, sessao.externalSessionId, sessao.sessionId, sessao.cwd]);

  async function abrirEm(pasta: string) {
    setOcupado(true); setFalha(null);
    try { await reabrirEmOutraPasta(sessao.sessionId, pasta); onReaberta(); }
    catch (motivo) { setFalha(motivo instanceof Error ? motivo.message : String(motivo)); }
    finally { setOcupado(false); }
  }

  const destinos = [...new Set([principal, sessao.projectPath].filter((p): p is string => Boolean(p) && p !== sessao.cwd))];
  return (
    <div>
      <p>{semCodigo(erro)}</p>
      <div className="mt-2 flex flex-wrap gap-2">
        {destinos.map((pasta) => (
          <Button key={pasta} size="sm" variant="glass" title={pasta} disabled={ocupado} onClick={() => void abrirEm(pasta)}>
            {pasta === principal ? "Abrir no repositório principal" : "Abrir na pasta do projeto"}
          </Button>
        ))}
      </div>
      {destinos.length === 0 && <p className="mt-2 text-text-muted">Abra o projeto de novo pela lateral e retome a conversa pelo Histórico.</p>}
      {falha && <p className="mt-2">{falha}</p>}
    </div>
  );
}

export function ConversaNaoAchada({ sessao, onNova, onFechar }: {
  sessao: SessaoDaAba;
  onNova: (terminalSessionId: string, titulo: string) => void;
  onFechar: () => void;
}) {
  const [ocupado, setOcupado] = useState(false);
  const [falha, setFalha] = useState<string | null>(null);
  const podeContinuar = Boolean(sessao.externalSessionId) && (sessao.provider === "claude" || sessao.provider === "codex");

  async function continuar() {
    if (!sessao.externalSessionId || !sessao.provider) return;
    setOcupado(true); setFalha(null);
    try {
      const nova = await continuarEmConversaNova({
        provider: sessao.provider, sessionId: sessao.externalSessionId, profileId: sessao.profileId, cwd: sessao.cwd,
        projectId: sessao.projectId, titulo: sessao.titulo,
      });
      if (nova.terminal_session_id) onNova(nova.terminal_session_id, `${sessao.titulo ?? "Conversa"} (continuação)`);
    } catch (motivo) {
      setFalha(motivo instanceof Error ? motivo.message : String(motivo));
    } finally { setOcupado(false); }
  }

  return (
    <div role="alert" className="absolute inset-x-3 top-3 z-10 border-2 border-warning bg-bg-elevated p-3 text-xs text-warning">
      <p>A CLI não conseguiu retomar esta conversa (não achou o histórico dela).</p>
      <div className="mt-2 flex flex-wrap gap-2">
        {podeContinuar && (
          <Button size="sm" variant="glass" disabled={ocupado} onClick={() => void continuar()}>
            Continuar numa conversa nova com o histórico
          </Button>
        )}
        <Button size="sm" variant="ghost" onClick={onFechar}>Fechar aviso</Button>
      </div>
      {falha && <p className="mt-2 text-danger">{falha}</p>}
    </div>
  );
}
