import { useCallback, useEffect, useMemo, useState } from "react";
import { createPortal } from "react-dom";
import { Button } from "../../components/ui";
import { projectName } from "../../lib/paths";
import { colecaoDoBruto, guardarBackup, listarBackups, registrarEventoDoWorkspace, type BackupDoWorkspace } from "../../services/backupDoWorkspace";
import { WORKSPACES_STORAGE_KEY } from "../../services/workspaceService";
import type { ProjetoParaAdotar, WorkspacesAction, WorkspaceState } from "../../types/workspace";
import { useAcoesDaPaleta } from "../paleta/acoes";
import { listConversations, type TerminalSession } from "../terminal/terminalService";
import { encontrarSessoesOrfas, projetosDasConversasRecentes, type ProjetoRecente } from "./recuperarWorkspace";

/**
 * Fork: "Recuperar workspace anterior" (paleta ⌘K e Configurações → Geral) e o aviso discreto de
 * agentes vivos fora de qualquer projeto aberto. Lógica pura em `recuperarWorkspace.ts`; backups em
 * `services/backupDoWorkspace.ts`.
 */

const EVENTO_ABRIR = "medza:recuperar-workspace";

/** Abre o diálogo de qualquer tela (as Configurações não têm o `dispatch` do workspace). */
export function abrirRecuperacaoDoWorkspace(): void {
  window.dispatchEvent(new Event(EVENTO_ABRIR));
}

interface Props {
  workspaces: WorkspaceState[];
  sessions: TerminalSession[];
  dispatch: React.Dispatch<WorkspacesAction>;
  /** Aviso das sessões órfãs só com o workspace na tela. */
  mostrarAviso: boolean;
}

export function RecuperacaoDoWorkspace({ workspaces, sessions, dispatch, mostrarAviso }: Props) {
  const [aberto, setAberto] = useState(false);
  const abrir = useCallback(() => setAberto(true), []);
  const fechar = useCallback(() => setAberto(false), []);
  useAcoesDaPaleta("recuperar-workspace", [{ id: "recuperar-workspace", titulo: "Recuperar workspace anterior", executar: abrir }]);

  useEffect(() => {
    window.addEventListener(EVENTO_ABRIR, abrir);
    return () => window.removeEventListener(EVENTO_ABRIR, abrir);
  }, [abrir]);

  const adotar = useCallback(
    (projetos: ProjetoParaAdotar[], origem: string) => {
      registrarEventoDoWorkspace(
        "workspace_projetos_adotados",
        `origem=${origem} projetos=${projetos.length} abas=${projetos.reduce((total, projeto) => total + projeto.abas.length, 0)}`
      );
      dispatch({ type: "ADOTAR_PROJETOS", projetos });
    },
    [dispatch]
  );

  return (
    <>
      {mostrarAviso && <AvisoDeSessoesOrfas workspaces={workspaces} sessions={sessions} onAdotar={(projetos) => adotar(projetos, "orfas")} />}
      {aberto && <DialogoDeRecuperacao workspaces={workspaces} sessions={sessions} dispatch={dispatch} onAdotar={adotar} onFechar={fechar} />}
    </>
  );
}

function AvisoDeSessoesOrfas({
  workspaces,
  sessions,
  onAdotar,
}: {
  workspaces: WorkspaceState[];
  sessions: TerminalSession[];
  onAdotar: (projetos: ProjetoParaAdotar[]) => void;
}) {
  const orfas = useMemo(() => encontrarSessoesOrfas(sessions, workspaces), [sessions, workspaces]);
  const assinatura = orfas.flatMap((projeto) => projeto.abas.map((aba) => aba.sessionId)).sort().join(",");
  // Dispensar vale para este conjunto de sessões: um agente órfão novo traz o aviso de volta.
  const [dispensado, setDispensado] = useState<string | null>(null);
  if (orfas.length === 0 || dispensado === assinatura) return null;
  const agentes = orfas.reduce((total, projeto) => total + projeto.abas.length, 0);
  const nomes = orfas.map((projeto) => projectName(projeto.path)).slice(0, 3).join(", ");

  return (
    <div role="status" aria-label="Agentes fora do workspace" className="fixed bottom-10 right-4 z-[90] max-w-sm border border-border-subtle bg-bg-elevated p-3 text-xs text-text-primary shadow-lg">
      <p>
        {agentes === 1 ? "1 agente rodando" : `${agentes} agentes rodando`} em {orfas.length === 1 ? "um projeto" : `${orfas.length} projetos`} fora
        do workspace: <strong>{nomes}</strong>
        {orfas.length > 3 ? "…" : ""}.
      </p>
      <div className="mt-2 flex justify-end gap-2">
        <Button variant="ghost" size="sm" onClick={() => setDispensado(assinatura)}>Dispensar</Button>
        <Button variant="primary" size="sm" onClick={() => onAdotar(orfas)}>
          Adicionar estes projetos ao workspace
        </Button>
      </div>
    </div>
  );
}

function dataLegivel(iso: string): string {
  const data = new Date(iso);
  return Number.isNaN(data.getTime()) ? iso : data.toLocaleString("pt-BR", { dateStyle: "short", timeStyle: "short" });
}

const MOTIVO: Record<string, string> = {
  conserto: "antes de um conserto automático",
  ilegivel: "estado ilegível",
  "antes-de-restaurar": "antes de restaurar outro",
};

function DialogoDeRecuperacao({
  workspaces,
  sessions,
  dispatch,
  onAdotar,
  onFechar,
}: {
  workspaces: WorkspaceState[];
  sessions: TerminalSession[];
  dispatch: React.Dispatch<WorkspacesAction>;
  onAdotar: (projetos: ProjetoParaAdotar[], origem: string) => void;
  onFechar: () => void;
}) {
  const [backups] = useState<BackupDoWorkspace[]>(() => listarBackups());
  const [recentes, setRecentes] = useState<ProjetoRecente[] | null>(null);
  const [buscando, setBuscando] = useState(false);
  const [erro, setErro] = useState<string | null>(null);

  useEffect(() => {
    const tecla = (event: KeyboardEvent) => { if (event.key === "Escape") onFechar(); };
    window.addEventListener("keydown", tecla);
    return () => window.removeEventListener("keydown", tecla);
  }, [onFechar]);

  function restaurar(backup: BackupDoWorkspace) {
    const colecao = colecaoDoBruto(backup.bruto);
    if (!colecao) {
      setErro("Esse backup não pôde ser lido.");
      return;
    }
    // O estado atual vira backup antes: restaurar também se desfaz.
    try {
      const atual = localStorage.getItem(WORKSPACES_STORAGE_KEY);
      if (atual) guardarBackup(WORKSPACES_STORAGE_KEY, atual, "antes-de-restaurar");
    } catch {
      /* sem armazenamento: restaura assim mesmo */
    }
    registrarEventoDoWorkspace("workspace_restaurado", `workspaces=${colecao.workspaces.length} projetos=${backup.projetos}`);
    dispatch({ type: "RESTAURAR_COLECAO", colecao });
    onFechar();
  }

  async function procurarConversas() {
    setBuscando(true);
    setErro(null);
    try {
      setRecentes(projetosDasConversasRecentes(await listConversations(), sessions, workspaces));
    } catch {
      setErro("Não deu para ler as conversas agora. O engine está rodando?");
    } finally {
      setBuscando(false);
    }
  }

  return createPortal(
    <div className="fixed inset-0 z-50 grid place-items-center bg-black/40 p-4" onPointerDown={(e) => { if (e.target === e.currentTarget) onFechar(); }}>
      <div role="dialog" aria-modal="true" aria-labelledby="recuperar-workspace-titulo" className="popover w-full max-w-lg p-4">
        <h2 id="recuperar-workspace-titulo" className="text-sm font-semibold text-text-primary">Recuperar workspace anterior</h2>

        <h3 className="mt-3 text-xs font-semibold text-text-secondary">Cópias guardadas</h3>
        {backups.length === 0 ? (
          <p className="mt-1 text-xs text-text-muted">Nenhuma cópia guardada. O app guarda uma sempre que precisa consertar o workspace salvo.</p>
        ) : (
          <ul className="mt-1 space-y-1">
            {backups.map((backup) => (
              <li key={backup.em + backup.chave} className="flex items-center justify-between gap-2 border border-border-subtle px-2 py-1.5 text-xs">
                <span className="min-w-0">
                  <span className="text-text-primary">{dataLegivel(backup.em)}</span>
                  <span className="text-text-muted">
                    {" · "}
                    {backup.projetos === 1 ? "1 projeto" : `${backup.projetos} projetos`}
                    {backup.workspaces > 1 ? ` em ${backup.workspaces} workspaces` : ""}
                    {" · "}
                    {MOTIVO[backup.motivo] ?? backup.motivo}
                  </span>
                </span>
                <Button size="sm" variant="glass" onClick={() => restaurar(backup)}>Restaurar</Button>
              </li>
            ))}
          </ul>
        )}

        <h3 className="mt-4 text-xs font-semibold text-text-secondary">Recriar a partir das conversas recentes</h3>
        <p className="mt-1 text-xs text-text-muted">Projetos das conversas das últimas 48 h que não estão em nenhum workspace, com as abas que o engine ainda tem.</p>
        {recentes === null ? (
          <Button className="mt-2" size="sm" variant="glass" loading={buscando} onClick={() => void procurarConversas()}>
            Procurar conversas recentes
          </Button>
        ) : recentes.length === 0 ? (
          <p className="mt-2 text-xs text-text-secondary">Nenhum projeto de fora: as conversas das últimas 48 h são de projetos que já estão abertos.</p>
        ) : (
          <>
            <ul className="mt-2 space-y-1">
              {recentes.map(({ projeto, conversas }) => (
                <li key={projeto.id} className="border border-border-subtle px-2 py-1.5 text-xs">
                  <span className="text-text-primary">{projectName(projeto.path)}</span>
                  <span className="text-text-muted">
                    {" · "}
                    {conversas === 1 ? "1 conversa" : `${conversas} conversas`}
                    {projeto.abas.length > 0 ? ` · ${projeto.abas.length === 1 ? "1 aba" : `${projeto.abas.length} abas`}` : ""}
                  </span>
                </li>
              ))}
            </ul>
            <Button
              className="mt-2"
              size="sm"
              variant="primary"
              onClick={() => {
                onAdotar(recentes.map((item) => item.projeto), "conversas");
                onFechar();
              }}
            >
              {recentes.length === 1 ? "Adicionar este projeto ao workspace" : `Adicionar ${recentes.length} projetos ao workspace`}
            </Button>
          </>
        )}

        {erro && <p role="alert" className="mt-2 text-xs text-danger">{erro}</p>}
        <div className="mt-4 flex justify-end">
          <Button variant="ghost" size="sm" onClick={onFechar}>Fechar</Button>
        </div>
      </div>
    </div>,
    document.body
  );
}
