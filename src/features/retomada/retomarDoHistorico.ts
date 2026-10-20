import type { WorkspaceState, WorkspacesAction } from "../../types/workspace";
import { collectResourceIds } from "../workspace/workspaceReducer";
import { ensureAgentTrust, ensureEngine, listAgentClis, spawnTerminal } from "../terminal/terminalService";
import { levarParaConta, resumeCommand, samePath, type HistoryEntry } from "../history/historyService";
import {
  continuarEmConversaNova,
  erroDePastaSumida,
  prepararRetomada,
  RetomadaError,
  type DiagnosticoDeRetomada,
  type OpcoesDeRetomada,
} from "./retomadaService";

/*
 * Fork (retomar-sempre): "Retomar no terminal" do Histórico. Antes, qualquer desvio virava um erro
 * sem saída (pasta não registrada, pasta apagada, conversa em outra conta, sessão já aberta em outra
 * aba). Agora, nesta ordem:
 * 1. conversa viva numa aba → foca a aba (não abre um segundo `--resume` da mesma conversa);
 * 2. pasta apagada → erro com "Abrir no repositório principal" / "Conversa nova lá, com o histórico";
 * 3. transcript em outra conta ou em outra pasta de projeto → o engine copia para onde a CLI procura;
 * 4. conversa que não existe em conta nenhuma → oferece conversa nova com o que houver de histórico.
 * Engine antigo (sem `/retomada/preparar`) segue o caminho de antes.
 */

export interface DependenciasDaRetomada {
  workspaces: WorkspaceState[];
  activeWorkspaceId: string | null;
  /** Projeto aberto agora: destino quando a conversa não registrou pasta. */
  projetoAtual: { id: string; path: string } | null;
  dispatch: (action: WorkspacesAction) => void;
  lembrarProjeto: (cwd: string) => void;
  /** Foi para a aba (troca para a vista de workspace). */
  aoAbrir: () => void;
}

function projetoDaPasta(workspaces: WorkspaceState[], cwd: string) {
  return workspaces
    .flatMap((item) => item.projects.map((project) => ({ workspaceId: item.id, project })))
    .find(({ project }) => samePath(project.path, cwd));
}

/** Onde focar uma sessão que já existe: o projeto cuja aba já mostra a sessão, senão o projeto dela. */
export function ondeFocar(workspaces: WorkspaceState[], sessionId: string, projectId: string | null) {
  for (const workspace of workspaces) {
    for (const project of workspace.projects) {
      if (collectResourceIds(project.layout).includes(sessionId)) return { workspaceId: workspace.id, projectId: project.id };
    }
  }
  const dono = workspaces.find((item) => item.projects.some((project) => project.id === projectId));
  return dono && projectId ? { workspaceId: dono.id, projectId } : null;
}

export async function retomarDoHistorico(
  entry: HistoryEntry,
  profileId: string,
  opcoes: OpcoesDeRetomada,
  deps: DependenciasDaRetomada,
): Promise<void> {
  const cli = (await listAgentClis()).find((item) => item.id === entry.provider);
  if (!cli?.available) {
    throw new RetomadaError(`${cli?.label ?? entry.provider} não foi encontrado no PATH. Instale a CLI (ou abra o app de novo depois de instalar) e tente outra vez.`);
  }
  const cwd = opcoes.cwd ?? entry.cwd;
  if (!cwd) {
    const atual = deps.projetoAtual;
    throw new RetomadaError(
      "Essa conversa não registrou a pasta onde rodou.",
      atual ? [{ rotulo: "Abrir no projeto aberto", dica: atual.path, opcoes: { cwd: atual.path } }] : [],
    );
  }
  await ensureEngine();

  let diagnostico: DiagnosticoDeRetomada | null = null;
  try {
    diagnostico = await prepararRetomada({
      provider: entry.provider, sessionId: entry.session_id, profileId, cwd, preparar: !opcoes.continuarNova,
    });
  } catch {
    diagnostico = null; // falha ao diagnosticar não impede tentar do jeito de antes
  }

  if (diagnostico?.viva) {
    const alvo = ondeFocar(deps.workspaces, diagnostico.viva.terminal_session_id, diagnostico.viva.project_id)
      ?? (() => { const achado = projetoDaPasta(deps.workspaces, cwd); return achado ? { workspaceId: achado.workspaceId, projectId: achado.project.id } : null; })();
    if (alvo) {
      deps.dispatch({ type: "FOCUS_SESSION", ...alvo, sessionId: diagnostico.viva.terminal_session_id, title: entry.title ?? cli.label });
      deps.aoAbrir();
      return;
    }
  }
  if (diagnostico && !diagnostico.pasta_existe) {
    throw erroDePastaSumida(`A pasta ${cwd} não existe mais (worktree apagada?).`, diagnostico.pasta_principal);
  }

  const found = projetoDaPasta(deps.workspaces, cwd);
  const workspaceId = found?.workspaceId ?? deps.activeWorkspaceId;
  if (!workspaceId) throw new RetomadaError("Crie um workspace antes de retomar a conversa.");
  // Id escolhido aqui pra já nascer a sessão no projeto certo, antes do reducer rodar.
  const projectId = found?.project.id ?? `project-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
  if (!found) {
    deps.lembrarProjeto(cwd);
    deps.dispatch({ type: "ADD_PROJECT", path: cwd, id: projectId });
  }

  if (opcoes.continuarNova) {
    const nova = await continuarEmConversaNova({
      provider: entry.provider, sessionId: entry.session_id, profileId, cwd, projectId, titulo: entry.title ?? entry.first_prompt,
    });
    if (nova.terminal_session_id) {
      deps.dispatch({ type: "FOCUS_SESSION", workspaceId, projectId, sessionId: nova.terminal_session_id, title: `${cli.label} · continuação` });
      deps.aoAbrir();
    }
    return;
  }

  if (diagnostico === null) {
    // Engine antigo: outra conta recebe o transcript antes, senão o `--resume` não acha a conversa.
    await levarParaConta(entry, profileId);
  } else if (!diagnostico.transcript) {
    throw new RetomadaError(
      "Não achei esta conversa em nenhuma conta (o arquivo dela foi apagado?).",
      [{ rotulo: "Abrir conversa nova nesta pasta", dica: cwd, opcoes: { cwd, continuarNova: true } }],
    );
  }
  await ensureAgentTrust(cli.id, cwd, profileId);
  const session = await spawnTerminal({
    projectId,
    name: `${cli.label} · ${(entry.title ?? "retomada").slice(0, 40)}`,
    cwd,
    rows: 30,
    cols: 120,
    initialCommand: resumeCommand(cli.command, entry),
    provider: cli.id,
    profileId,
    externalSessionId: entry.session_id,
  });
  deps.dispatch({ type: "FOCUS_SESSION", workspaceId, projectId, sessionId: session.id, title: session.name });
  deps.aoAbrir();
}
