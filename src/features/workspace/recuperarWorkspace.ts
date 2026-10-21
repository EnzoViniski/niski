import { samePath } from "../../lib/paths";
import { PASTA_DAS_WORKTREES } from "../terminal/worktreeIsolada";
import type { Conversation, TerminalSession } from "../terminal/terminalService";
import type {
  LayoutNode,
  PaneNode,
  ProjetoParaAdotar,
  WorkspaceCollection,
  WorkspaceState,
  WorkspaceTab,
} from "../../types/workspace";
import { collectResourceIds, createProject } from "./workspaceReducer";
// Fork: conversa do editor de vídeo não é projeto solto (ver features/video/ChatDoEditor.tsx).
import { ehConversaDoVideo } from "../video/projetoDoVideo";

/**
 * Fork: recuperar projetos que sumiram do workspace usando só o que a UI já recebe do engine
 * (lista de sessões e de conversas) — nunca lendo `sessions.json`/`conversations.json` direto.
 *
 * - Sessões órfãs: agente vivo cujo `project_id` não é de nenhum projeto aberto e que não está em
 *   nenhuma aba. A lateral filtra por `project_id`, então essas sessões ficam invisíveis.
 * - Conversas recentes: projetos das conversas das últimas 48 h que não estão em workspace nenhum.
 *
 * O projeto volta com o **mesmo id** das sessões, e a lateral volta a listar as conversas dele.
 */

export const JANELA_DAS_CONVERSAS_MS = 48 * 60 * 60 * 1000;
const ABAS_POR_PROJETO = 6;
const ESTADOS_VIVOS = new Set<TerminalSession["state"]>(["working", "answered", "approval_required"]);

let sequencia = 0;
function idDaAba(): string {
  sequencia += 1;
  return `tab-adotada-${Date.now().toString(36)}-${sequencia.toString(36)}`;
}

/** Pasta do projeto a partir do `cwd` da sessão: worktree isolada (`<pai>/.medza-worktrees/<repo>/<nome>`)
 *  volta para o repositório (`<pai>/<repo>`); o resto fica como está. */
export function pastaDoProjeto(cwd: string): string {
  const partes = cwd.split(/[\\/]/);
  const i = partes.lastIndexOf(PASTA_DAS_WORKTREES);
  if (i < 0 || !partes[i + 1] || !partes[i + 2]) return cwd;
  const separador = cwd.includes("\\") && !cwd.includes("/") ? "\\" : "/";
  return [...partes.slice(0, i), partes[i + 1]].join(separador);
}

function todosOsProjetos(workspaces: WorkspaceState[]) {
  return workspaces.flatMap((workspace) => workspace.projects);
}

function recursosAbertos(workspaces: WorkspaceState[]): Set<string> {
  return new Set(todosOsProjetos(workspaces).flatMap((projeto) => collectResourceIds(projeto.layout)));
}

function projetoConhecido(workspaces: WorkspaceState[], id: string, path: string): boolean {
  return todosOsProjetos(workspaces).some((projeto) => projeto.id === id || samePath(projeto.path, path));
}

/** Agentes vivos fora de qualquer projeto aberto e de qualquer aba, agrupados por projeto. */
export function encontrarSessoesOrfas(sessions: TerminalSession[], workspaces: WorkspaceState[]): ProjetoParaAdotar[] {
  const abertos = recursosAbertos(workspaces);
  const grupos = new Map<string, ProjetoParaAdotar>();
  for (const sessao of sessions) {
    if (!sessao.provider || !ESTADOS_VIVOS.has(sessao.state) || !sessao.cwd || abertos.has(sessao.id)) continue;
    if (ehConversaDoVideo(sessao.project_id)) continue; // Fork: mora no editor de vídeo.
    const path = pastaDoProjeto(sessao.cwd);
    // Projeto (ou pasta) já aberto em algum workspace: não é órfã.
    if (projetoConhecido(workspaces, sessao.project_id, path)) continue;
    const chave = sessao.project_id || `pasta:${path}`;
    const grupo = grupos.get(chave) ?? { id: sessao.project_id || `project-adotado-${grupos.size}-${Date.now().toString(36)}`, path, abas: [] };
    if (grupo.abas.length < ABAS_POR_PROJETO) {
      grupo.abas.push({ sessionId: sessao.id, title: sessao.name || sessao.provider, provider: sessao.provider });
    }
    grupos.set(chave, grupo);
  }
  return [...grupos.values()];
}

function ultimaAtividade(conversa: Conversation): number {
  return Math.max(
    conversa.created_at_ms ?? 0,
    ...(conversa.segments ?? []).flatMap((segmento) => [segmento.started_at_ms ?? 0, segmento.ended_at_ms ?? 0])
  );
}

export interface ProjetoRecente {
  projeto: ProjetoParaAdotar;
  conversas: number;
  ultimaMs: number;
}

/** Projetos das conversas das últimas 48 h que não estão em nenhum workspace. As abas são as sessões
 *  dessas conversas que o engine ainda conhece (a aba reabre a conversa como estava). */
export function projetosDasConversasRecentes(
  conversas: Conversation[],
  sessions: TerminalSession[],
  workspaces: WorkspaceState[],
  agora = Date.now(),
  janelaMs = JANELA_DAS_CONVERSAS_MS
): ProjetoRecente[] {
  const sessoesPorId = new Map(sessions.map((sessao) => [sessao.id, sessao]));
  const abertos = recursosAbertos(workspaces);
  const recentes = conversas
    .map((conversa) => ({ conversa, em: ultimaAtividade(conversa) }))
    .filter(({ conversa, em }) => conversa.cwd && em >= agora - janelaMs && !ehConversaDoVideo(conversa.project_id)) // Fork: vídeo fica no editor.
    .sort((a, b) => b.em - a.em);
  const grupos = new Map<string, ProjetoRecente>();
  for (const { conversa, em } of recentes) {
    const path = pastaDoProjeto(conversa.cwd);
    if (projetoConhecido(workspaces, conversa.project_id, path)) continue;
    // Mesma pasta com ids diferentes vira um projeto só (o do uso mais recente).
    const chave = [...grupos.keys()].find((outra) => samePath(grupos.get(outra)!.projeto.path, path)) ?? (conversa.project_id || path);
    const grupo = grupos.get(chave) ?? {
      projeto: { id: conversa.project_id || `project-recriado-${grupos.size}-${Date.now().toString(36)}`, path, abas: [] },
      conversas: 0,
      ultimaMs: em,
    };
    grupo.conversas += 1;
    const segmento = [...(conversa.segments ?? [])].reverse().find((item) => item.terminal_session_id);
    const sessao = segmento?.terminal_session_id ? sessoesPorId.get(segmento.terminal_session_id) : undefined;
    if (
      sessao &&
      sessao.state !== "orphan" &&
      !abertos.has(sessao.id) &&
      grupo.projeto.abas.length < ABAS_POR_PROJETO &&
      !grupo.projeto.abas.some((aba) => aba.sessionId === sessao.id)
    ) {
      grupo.projeto.abas.push({ sessionId: sessao.id, title: conversa.title || sessao.name || segmento!.provider, provider: segmento!.provider });
    }
    grupos.set(chave, grupo);
  }
  return [...grupos.values()];
}

function abaDaSessao(aba: ProjetoParaAdotar["abas"][number]): WorkspaceTab {
  const tab: WorkspaceTab = { id: idDaAba(), kind: "agent", title: aba.title, resourceId: aba.sessionId };
  return aba.provider ? { ...tab, provider: aba.provider } : tab;
}

function acrescentarNoPainel(no: LayoutNode, paneId: string, abas: WorkspaceTab[]): LayoutNode {
  if (no.type === "pane") {
    return no.id === paneId ? { ...no, tabs: [...no.tabs, ...abas], activeTabId: abas[0]?.id ?? no.activeTabId } : no;
  }
  return { ...no, first: acrescentarNoPainel(no.first, paneId, abas), second: acrescentarNoPainel(no.second, paneId, abas) };
}

/** Põe os projetos no workspace ativo: projeto que já está lá (mesmo id ou pasta) ganha só as abas
 *  que faltam; os outros nascem com as abas das sessões no lugar do "novo agente". */
export function adotarProjetos(colecao: WorkspaceCollection, projetos: ProjetoParaAdotar[]): WorkspaceCollection {
  if (projetos.length === 0) return colecao;
  const alvo = colecao.workspaces.find((workspace) => workspace.id === colecao.activeWorkspaceId) ?? colecao.workspaces[0];
  if (!alvo) return colecao;
  const jaAbertos = recursosAbertos(colecao.workspaces);
  let workspace = alvo;
  for (const projeto of projetos) {
    const abas = projeto.abas.filter((aba) => !jaAbertos.has(aba.sessionId)).map(abaDaSessao);
    abas.forEach((aba) => jaAbertos.add(aba.resourceId!));
    const existente = workspace.projects.find((item) => item.id === projeto.id || samePath(item.path, projeto.path));
    if (existente) {
      if (abas.length === 0) continue;
      workspace = {
        ...workspace,
        projects: workspace.projects.map((item) =>
          item.id === existente.id ? { ...item, layout: acrescentarNoPainel(item.layout, item.activePaneId, abas) } : item
        ),
      };
      continue;
    }
    const novo = createProject(projeto.path, projeto.id);
    const layout: PaneNode | LayoutNode =
      abas.length > 0 && novo.layout.type === "pane" ? { ...novo.layout, tabs: abas, activeTabId: abas[0].id } : novo.layout;
    workspace = { ...workspace, projects: [...workspace.projects, { ...novo, layout }] };
  }
  const primeiro = workspace.projects.find((item) => projetos.some((projeto) => projeto.id === item.id || samePath(item.path, projeto.path)));
  workspace = { ...workspace, activeProjectId: primeiro?.id ?? workspace.activeProjectId, updatedAt: new Date().toISOString() };
  return {
    ...colecao,
    activeWorkspaceId: workspace.id,
    workspaces: colecao.workspaces.map((item) => (item.id === alvo.id ? workspace : item)),
  };
}

/** A coleção do backup já vem consertada (`colecaoDoBruto`); aqui só garante um ativo válido. */
export function restaurarColecao(colecao: WorkspaceCollection): WorkspaceCollection {
  const ativo = colecao.workspaces.some((workspace) => workspace.id === colecao.activeWorkspaceId)
    ? colecao.activeWorkspaceId
    : (colecao.workspaces[0]?.id ?? null);
  return { ...colecao, activeWorkspaceId: ativo };
}

/*
 * Fork (atualização automática): depois de `reiniciar_para_atualizar` com o engine trocado, os agentes
 * voltam como sessões históricas (`retoma_ao_abrir`). As abas do projeto na tela já se religam sozinhas
 * (o `TerminalPane` montado chama `restart_terminal` na primeira leitura); as abas visíveis dos outros
 * workspaces (e painéis escondidos por um maximizado) só voltariam no clique. Aqui elas são religadas
 * uma vez, só nesta abertura — a abertura normal não passa por aqui (`consumirReinicioDeAtualizacao`).
 */

function noPorId(no: LayoutNode, id: string): LayoutNode | null {
  if (no.id === id) return no;
  return no.type === "pane" ? null : (noPorId(no.first, id) ?? noPorId(no.second, id));
}

/** Sessão da aba que aparece em cada painel (a mesma regra do `PaneView`). */
function sessoesDasAbasAtivas(no: LayoutNode): string[] {
  if (no.type !== "pane") return [...sessoesDasAbasAtivas(no.first), ...sessoesDasAbasAtivas(no.second)];
  const aba = no.tabs.find((item) => item.id === no.activeTabId) ?? no.tabs[0];
  return aba?.kind === "agent" && aba.resourceId ? [aba.resourceId] : [];
}

/** Abas visíveis de cada workspace (projeto ativo de cada um), menos as montadas agora na tela. */
export function abasVisiveisParaRetomar(colecao: WorkspaceCollection): string[] {
  const ativo = colecao.workspaces.find((workspace) => workspace.id === colecao.activeWorkspaceId) ?? colecao.workspaces[0];
  const naTela = ativo?.projects.find((projeto) => projeto.id === ativo.activeProjectId);
  // Painel maximizado: só ele está montado.
  const montado = naTela ? ((naTela.maximizedPaneId && noPorId(naTela.layout, naTela.maximizedPaneId)) || naTela.layout) : null;
  const montadas = new Set(montado ? sessoesDasAbasAtivas(montado) : []);
  const visiveis = colecao.workspaces.flatMap((workspace) => {
    const projeto = workspace.projects.find((item) => item.id === workspace.activeProjectId);
    return projeto ? sessoesDasAbasAtivas(projeto.layout) : [];
  });
  return [...new Set(visiveis)].filter((id) => !montadas.has(id));
}

export interface DependenciasDaRetomadaAposAtualizar {
  listar: () => Promise<TerminalSession[]>;
  reiniciar: (sessionId: string) => Promise<unknown>;
  esperar?: (ms: number) => Promise<void>;
  tentativas?: number;
}

/** Espera o engine novo responder e religa (`--resume`) as abas visíveis que ele marcou para retomar.
 *  Só mexe em sessão `retoma_ao_abrir`: agente já vivo nunca é reiniciado. Devolve as religadas. */
export async function retomarAbasVisiveisAposAtualizar(colecao: WorkspaceCollection, deps: DependenciasDaRetomadaAposAtualizar): Promise<string[]> {
  const alvo = new Set(abasVisiveisParaRetomar(colecao));
  if (alvo.size === 0) return [];
  const esperar = deps.esperar ?? ((ms: number) => new Promise<void>((resolve) => setTimeout(resolve, ms)));
  let sessoes: TerminalSession[] | null = null;
  for (let tentativa = 0; tentativa < (deps.tentativas ?? 20) && sessoes === null; tentativa += 1) {
    if (tentativa > 0) await esperar(1_500);
    sessoes = await deps.listar().catch(() => null);
  }
  const religadas: string[] = [];
  for (const sessao of sessoes ?? []) {
    if (!alvo.has(sessao.id) || !sessao.retoma_ao_abrir) continue;
    // Uma por vez: cada uma sobe um shell e a CLI.
    if (await deps.reiniciar(sessao.id).then(() => true, () => false)) religadas.push(sessao.id);
  }
  return religadas;
}
