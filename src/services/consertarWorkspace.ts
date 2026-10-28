import type {
  LayoutNode,
  PaneKind,
  PaneNode,
  Project,
  WorkspaceCollection,
  WorkspaceState,
  WorkspaceTab,
} from "../types/workspace";

/**
 * Leitura tolerante do workspace salvo no localStorage.
 *
 * Antes a validação era tudo-ou-nada: uma única aba com `kind` fora da lista (a "orquestracao",
 * que entrou no tipo e não na lista) derrubava o workspace inteiro, e o vazio era gravado por cima
 * no próximo save. Aqui cada nível conserta o que dá e só descarta a menor unidade quebrada:
 *
 * - aba sem id/título/kind em texto: sai sozinha;
 * - aba com `kind` desconhecido (de uma versão mais nova do app): **fica como está**. O `PaneView`
 *   já cai num painel neutro para kind que não conhece ("Aba de uma versão mais nova") e o ícone cai
 *   no do caminho/título, então nada quebra; e ao voltar para a versão nova a aba reaparece intacta.
 *   Trocar por um placeholder perderia o `resourceId` e o kind de verdade para sempre;
 * - `activeTabId`/`activePaneId`/`maximizedPaneId` que não apontam para nada: reapontados;
 * - `ratio` fora de 0.2–0.8 (ou não numérico): grampeado (0.5 se nem número for);
 * - split com um lado ilegível: vira o lado que sobrou;
 * - projeto: só cai sem `id` ou `path`; layout ilegível vira um painel vazio;
 * - workspace: só cai se não for objeto.
 *
 * O relatório conta o que mudou (só números e kinds) para o backup e o log.
 */

/** Fonte única dos kinds de aba. O `satisfies` obriga a cobrir todo `PaneKind`: kind novo no tipo
 *  sem entrada aqui é erro de compilação (e o teste `consertarWorkspace.test.ts` confere também). */
const KINDS_CONHECIDOS = {
  "conversa-historica": true,
  agent: true,
  terminal: true,
  command: true,
  file: true,
  markdown: true,
  "git-diff": true,
  revisao: true,
  "git-graph": true,
  kanban: true,
  logs: true,
  "docker-logs": true,
  web: true,
  orquestracao: true,
} as const satisfies Record<PaneKind, true>;

export const PANE_KINDS: ReadonlySet<string> = new Set(Object.keys(KINDS_CONHECIDOS));

export function ehKindConhecido(kind: unknown): kind is PaneKind {
  return typeof kind === "string" && PANE_KINDS.has(kind);
}

export const RATIO_MINIMO = 0.2;
export const RATIO_MAXIMO = 0.8;

export interface RelatorioDoConserto {
  workspacesDescartados: number;
  projetosDescartados: number;
  abasDescartadas: number;
  /** Abas preservadas com kind que esta versão não conhece (não conta como conserto). */
  abasDeVersaoNova: number;
  kindsDesconhecidos: string[];
  idsReapontados: number;
  ratiosGrampeados: number;
  nosDescartados: number;
  camposCorrigidos: number;
}

export function relatorioVazio(): RelatorioDoConserto {
  return {
    workspacesDescartados: 0,
    projetosDescartados: 0,
    abasDescartadas: 0,
    abasDeVersaoNova: 0,
    kindsDesconhecidos: [],
    idsReapontados: 0,
    ratiosGrampeados: 0,
    nosDescartados: 0,
    camposCorrigidos: 0,
  };
}

/** Houve descarte ou conserto (aba de versão nova preservada não conta: o dado não muda). */
export function houveConserto(relatorio: RelatorioDoConserto): boolean {
  return (
    relatorio.workspacesDescartados +
      relatorio.projetosDescartados +
      relatorio.abasDescartadas +
      relatorio.idsReapontados +
      relatorio.ratiosGrampeados +
      relatorio.nosDescartados +
      relatorio.camposCorrigidos >
    0
  );
}

const ehObjeto = (valor: unknown): valor is Record<string, unknown> =>
  Boolean(valor) && typeof valor === "object" && !Array.isArray(valor);
const ehTextoComConteudo = (valor: unknown): valor is string => typeof valor === "string" && valor.length > 0;

let sequencia = 0;
function idRecuperado(prefixo: string): string {
  sequencia += 1;
  return `${prefixo}-recuperado-${Date.now().toString(36)}-${sequencia.toString(36)}`;
}

function nomeDaPasta(path: string): string {
  return path.split(/[\\/]/).filter(Boolean).pop() ?? path;
}

/** Kind só entra no relatório como texto curto e simples: é metadado, nunca conteúdo. */
function kindParaRelatorio(kind: string): string {
  return kind.replace(/[^a-z0-9-]/gi, "").slice(0, 24) || "?";
}

function consertarAba(valor: unknown, r: RelatorioDoConserto): WorkspaceTab | null {
  if (!ehObjeto(valor) || !ehTextoComConteudo(valor.id) || typeof valor.title !== "string" || !ehTextoComConteudo(valor.kind)) {
    r.abasDescartadas += 1;
    return null;
  }
  const aba = { ...valor } as Record<string, unknown>;
  for (const campo of ["resourceId", "provider", "projectId"]) {
    if (campo in aba && aba[campo] !== undefined && typeof aba[campo] !== "string") {
      delete aba[campo];
      r.camposCorrigidos += 1;
    }
  }
  if (!ehKindConhecido(valor.kind)) {
    r.abasDeVersaoNova += 1;
    const kind = kindParaRelatorio(valor.kind);
    if (!r.kindsDesconhecidos.includes(kind) && r.kindsDesconhecidos.length < 5) r.kindsDesconhecidos.push(kind);
  }
  return aba as unknown as WorkspaceTab;
}

function consertarNo(valor: unknown, r: RelatorioDoConserto): LayoutNode | null {
  if (!ehObjeto(valor)) {
    r.nosDescartados += 1;
    return null;
  }
  if (valor.type === "pane") {
    const id = ehTextoComConteudo(valor.id) ? valor.id : (r.camposCorrigidos++, idRecuperado("pane"));
    if (!Array.isArray(valor.tabs)) r.camposCorrigidos += 1;
    const tabs = (Array.isArray(valor.tabs) ? valor.tabs : [])
      .map((aba) => consertarAba(aba, r))
      .filter((aba): aba is WorkspaceTab => aba !== null);
    let activeTabId: string | null;
    if (tabs.length === 0) {
      activeTabId = null;
      if (valor.activeTabId !== null) r.idsReapontados += 1;
    } else if (typeof valor.activeTabId === "string" && tabs.some((aba) => aba.id === valor.activeTabId)) {
      activeTabId = valor.activeTabId;
    } else {
      activeTabId = tabs[0].id;
      r.idsReapontados += 1;
    }
    return { ...valor, type: "pane", id, tabs, activeTabId } as PaneNode;
  }
  if (valor.type === "split") {
    const first = consertarNo(valor.first, r);
    const second = consertarNo(valor.second, r);
    // Um lado ilegível: o split vira o lado que sobrou (como fechar aquele painel).
    if (!first || !second) return first ?? second;
    const id = ehTextoComConteudo(valor.id) ? valor.id : (r.camposCorrigidos++, idRecuperado("split"));
    const direction = valor.direction === "horizontal" || valor.direction === "vertical" ? valor.direction : (r.camposCorrigidos++, "horizontal" as const);
    let ratio: number;
    if (typeof valor.ratio !== "number" || !Number.isFinite(valor.ratio)) {
      ratio = 0.5;
      r.ratiosGrampeados += 1;
    } else if (valor.ratio < RATIO_MINIMO || valor.ratio > RATIO_MAXIMO) {
      ratio = Math.min(RATIO_MAXIMO, Math.max(RATIO_MINIMO, valor.ratio));
      r.ratiosGrampeados += 1;
    } else {
      ratio = valor.ratio;
    }
    return { ...valor, type: "split", id, direction, ratio, first, second } as LayoutNode;
  }
  r.nosDescartados += 1;
  return null;
}

function idsDosPaineis(no: LayoutNode): string[] {
  return no.type === "pane" ? [no.id] : [...idsDosPaineis(no.first), ...idsDosPaineis(no.second)];
}

function consertarProjeto(valor: unknown, r: RelatorioDoConserto): Project | null {
  if (!ehObjeto(valor) || !ehTextoComConteudo(valor.id) || !ehTextoComConteudo(valor.path)) {
    r.projetosDescartados += 1;
    return null;
  }
  let layout = consertarNo(valor.layout, r);
  if (!layout) {
    layout = { type: "pane", id: idRecuperado("pane"), tabs: [], activeTabId: null };
    r.camposCorrigidos += 1;
  }
  const paineis = idsDosPaineis(layout);
  let activePaneId: string;
  if (typeof valor.activePaneId === "string" && paineis.includes(valor.activePaneId)) {
    activePaneId = valor.activePaneId;
  } else {
    activePaneId = paineis[0];
    r.idsReapontados += 1;
  }
  let maximizedPaneId: string | null = null;
  if (typeof valor.maximizedPaneId === "string" && paineis.includes(valor.maximizedPaneId)) {
    maximizedPaneId = valor.maximizedPaneId;
  } else if (valor.maximizedPaneId != null) {
    r.idsReapontados += 1;
  }
  const textoOuNulo = (campo: string): string | null => {
    const atual = valor[campo];
    if (atual === null || typeof atual === "string") return atual;
    if (atual !== undefined) r.camposCorrigidos += 1;
    return null;
  };
  const name = typeof valor.name === "string" && valor.name ? valor.name : (r.camposCorrigidos++, nomeDaPasta(valor.path));
  return {
    ...valor,
    id: valor.id,
    name,
    path: valor.path,
    gitRoot: textoOuNulo("gitRoot"),
    branch: textoOuNulo("branch"),
    composeFile: textoOuNulo("composeFile"),
    lastOpenedAt: typeof valor.lastOpenedAt === "string" ? valor.lastOpenedAt : new Date(0).toISOString(),
    layout,
    activePaneId,
    maximizedPaneId,
  } as Project;
}

/** Conserta um workspace; `null` só se não for objeto. `indice` dá nome a quem perdeu o seu. */
export function consertarWorkspace(valor: unknown, r: RelatorioDoConserto, indice = 0): WorkspaceState | null {
  if (!ehObjeto(valor)) {
    r.workspacesDescartados += 1;
    return null;
  }
  if (valor.version !== 1) r.camposCorrigidos += 1;
  const id = ehTextoComConteudo(valor.id) ? valor.id : (r.camposCorrigidos++, idRecuperado("workspace"));
  const name = typeof valor.name === "string" && valor.name ? valor.name : (r.camposCorrigidos++, `Workspace ${indice + 1}`);
  if (!Array.isArray(valor.projects)) r.camposCorrigidos += 1;
  const projects = (Array.isArray(valor.projects) ? valor.projects : [])
    .map((projeto) => consertarProjeto(projeto, r))
    .filter((projeto): projeto is Project => projeto !== null);
  let activeProjectId: string | null;
  if (typeof valor.activeProjectId === "string" && projects.some((projeto) => projeto.id === valor.activeProjectId)) {
    activeProjectId = valor.activeProjectId;
  } else {
    activeProjectId = projects[0]?.id ?? null;
    if (valor.activeProjectId !== activeProjectId) r.idsReapontados += 1;
  }
  return {
    ...valor,
    version: 1,
    id,
    name,
    projects,
    activeProjectId,
    updatedAt: typeof valor.updatedAt === "string" ? valor.updatedAt : new Date().toISOString(),
  } as WorkspaceState;
}

/** Conserta a coleção; `null` quando nem a lista de workspaces existe (aí vale a chave legada). */
export function consertarColecao(valor: unknown, r: RelatorioDoConserto): WorkspaceCollection | null {
  if (!ehObjeto(valor) || !Array.isArray(valor.workspaces)) return null;
  if (valor.version !== 1) r.camposCorrigidos += 1;
  const workspaces = valor.workspaces
    .map((workspace, indice) => consertarWorkspace(workspace, r, indice))
    .filter((workspace): workspace is WorkspaceState => workspace !== null);
  if (workspaces.length === 0) {
    // Lista vazia salva (ou toda ilegível): o app nunca fica sem workspace nenhum.
    return { version: 1, workspaces: [workspaceVazio()], activeWorkspaceId: null };
  }
  const activeWorkspaceId = workspaces.some((workspace) => workspace.id === valor.activeWorkspaceId)
    ? (valor.activeWorkspaceId as string)
    : (r.idsReapontados++, workspaces[0].id);
  return { version: 1, workspaces, activeWorkspaceId };
}

export function workspaceVazio(): WorkspaceState {
  return {
    version: 1,
    id: "default",
    name: "Workspace",
    projects: [],
    activeProjectId: null,
    updatedAt: new Date().toISOString(),
  };
}

/** Só números e kinds: é o `detalhe` do evento no log em disco. */
export function resumoDoRelatorio(r: RelatorioDoConserto, extra: Record<string, string | number> = {}): string {
  const partes = [
    `workspaces_descartados=${r.workspacesDescartados}`,
    `projetos_descartados=${r.projetosDescartados}`,
    `abas_descartadas=${r.abasDescartadas}`,
    `abas_versao_nova=${r.abasDeVersaoNova}`,
    `ids_reapontados=${r.idsReapontados}`,
    `ratios_grampeados=${r.ratiosGrampeados}`,
    `nos_descartados=${r.nosDescartados}`,
    `campos_corrigidos=${r.camposCorrigidos}`,
    ...Object.entries(extra).map(([chave, valor]) => `${chave}=${valor}`),
  ];
  if (r.kindsDesconhecidos.length) partes.push(`kinds=${r.kindsDesconhecidos.join(",")}`);
  return partes.join(" ");
}
