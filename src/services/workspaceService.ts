import type { WorkspaceCollection, WorkspaceState } from "../types/workspace";
// Leitura tolerante (conserta em vez de descartar) e backup do JSON cru antes de mexer.
import {
  consertarColecao,
  consertarWorkspace,
  houveConserto,
  relatorioVazio,
  resumoDoRelatorio,
  workspaceVazio,
  type RelatorioDoConserto,
} from "./consertarWorkspace";
import { guardarBackup, registrarEventoDoWorkspace } from "./backupDoWorkspace";

export const WORKSPACE_STORAGE_KEY = "omni-agents-workspace";
// A lista de kinds mora em `consertarWorkspace.ts` (fonte única tipada por `PaneKind`).
export { PANE_KINDS } from "./consertarWorkspace";

export function emptyWorkspace(): WorkspaceState {
  return workspaceVazio();
}

/** Lê o JSON cru; `undefined` = chave ausente, `{ ilegivel }` = texto que não é JSON. */
function lerBruto(chave: string): { bruto: string; valor: unknown; ilegivel: boolean } | undefined {
  let bruto: string | null;
  try {
    bruto = localStorage.getItem(chave);
  } catch {
    return undefined;
  }
  if (!bruto) return undefined;
  try {
    return { bruto, valor: JSON.parse(bruto), ilegivel: false };
  } catch {
    return { bruto, valor: undefined, ilegivel: true };
  }
}

/** Antes de qualquer descarte ou conserto, guarda o original e registra só as contagens. */
function avisarConserto(chave: string, bruto: string, relatorio: RelatorioDoConserto | null): void {
  if (!relatorio) {
    guardarBackup(chave, bruto, "ilegivel");
    registrarEventoDoWorkspace("workspace_ilegivel", `chave=${chave} bytes=${bruto.length}`);
    return;
  }
  if (houveConserto(relatorio)) {
    guardarBackup(chave, bruto, "conserto");
    registrarEventoDoWorkspace("workspace_consertado", resumoDoRelatorio(relatorio, { chave }));
  } else if (relatorio.abasDeVersaoNova > 0) {
    // Aba de versão mais nova fica intacta (não é conserto), mas vale saber que apareceu.
    registrarEventoDoWorkspace("workspace_aba_desconhecida", resumoDoRelatorio(relatorio, { chave }));
  }
}

export function loadWorkspace(): WorkspaceState {
  const lido = lerBruto(WORKSPACE_STORAGE_KEY);
  if (!lido) return emptyWorkspace();
  if (lido.ilegivel) {
    avisarConserto(WORKSPACE_STORAGE_KEY, lido.bruto, null);
    return emptyWorkspace();
  }
  const relatorio = relatorioVazio();
  const workspace = consertarWorkspace(lido.valor, relatorio);
  avisarConserto(WORKSPACE_STORAGE_KEY, lido.bruto, relatorio);
  return workspace ?? emptyWorkspace();
}

export function saveWorkspace(workspace: WorkspaceState): void {
  localStorage.setItem(WORKSPACE_STORAGE_KEY, JSON.stringify(workspace));
}

export const WORKSPACES_STORAGE_KEY = "omni-agents-workspaces";

function migrateLegacyWorkspace(): WorkspaceCollection {
  const legacy = loadWorkspace();
  const workspace =
    legacy.id === "default" && legacy.name === "Workspace" ? { ...legacy, name: "Workspace 1" } : legacy;
  return { version: 1, workspaces: [workspace], activeWorkspaceId: workspace.id };
}

export function loadWorkspaces(): WorkspaceCollection {
  const lido = lerBruto(WORKSPACES_STORAGE_KEY);
  if (lido) {
    // Cada nível conserta o que dá (`consertarColecao`); só sem lista de workspaces nenhuma
    // é que cai na chave legada — e aí o original já ficou guardado no backup.
    const relatorio = relatorioVazio();
    const colecao = lido.ilegivel ? null : consertarColecao(lido.valor, relatorio);
    if (colecao) {
      avisarConserto(WORKSPACES_STORAGE_KEY, lido.bruto, relatorio);
      return colecao;
    }
    avisarConserto(WORKSPACES_STORAGE_KEY, lido.bruto, null);
  }
  return migrateLegacyWorkspace();
}

export function saveWorkspaces(collection: WorkspaceCollection): void {
  localStorage.setItem(WORKSPACES_STORAGE_KEY, JSON.stringify(collection));
}
