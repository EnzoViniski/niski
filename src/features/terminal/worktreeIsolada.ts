import { invoke } from "@tauri-apps/api/core";

/**
 * Fork: agente em worktree isolada do git (Novo agente → "Isolar em worktree"; também o `criar_agente`
 * do orquestrador com `worktree`). A worktree fica **fora** do repositório, ao lado dele:
 * `<pai do repo>/.medza-worktrees/<repo>/<nome>`, na branch `medza/<nome>`. Lógica de git em
 * `crates/omni-core/src/worktree_isolada.rs`.
 */

export const PASTA_DAS_WORKTREES = ".medza-worktrees";
export const PREFIXO_DA_BRANCH = "medza/";

export interface WorktreeIsolada {
  caminho: string;
  /** Onde o agente roda (a raiz, ou a mesma subpasta de onde ele saiu). */
  pasta_de_trabalho: string;
  branch: string;
  nome: string;
  copiados: number;
}

export interface EstadoDaWorktree {
  caminho: string;
  branch: string;
  mudancas: number;
  exemplos: string[];
}

export function criarWorktreeIsolada(projectPath: string, nome: string): Promise<WorktreeIsolada> {
  return invoke<WorktreeIsolada>("criar_worktree_isolada", { projectPath, nome });
}

export function estadoDaWorktreeIsolada(caminho: string): Promise<EstadoDaWorktree | null> {
  return invoke<EstadoDaWorktree | null>("estado_da_worktree_isolada", { caminho });
}

export function removerWorktreeIsolada(caminho: string, forcar: boolean): Promise<void> {
  return invoke<void>("remover_worktree_isolada", { caminho, forcar });
}

/** `{ repo, nome, branch, raiz }` se a pasta está numa worktree isolada; senão `null`. Só pelo caminho
 *  (sem git): é o que a aba usa para mostrar a branch sem custo. */
export function worktreeDaPasta(cwd: string | null | undefined): { repo: string; nome: string; branch: string; raiz: string } | null {
  if (!cwd) return null;
  const partes = cwd.split(/[\\/]/);
  const i = partes.lastIndexOf(PASTA_DAS_WORKTREES);
  if (i < 0 || !partes[i + 1] || !partes[i + 2]) return null;
  const separador = cwd.includes("\\") && !cwd.includes("/") ? "\\" : "/";
  return {
    repo: partes[i + 1],
    nome: partes[i + 2],
    branch: `${PREFIXO_DA_BRANCH}${partes[i + 2]}`,
    raiz: partes.slice(0, i + 3).join(separador),
  };
}

/** Prévia do nome como o Rust vai gravar (`nome_valido`): minúsculas, sem acento, `-` no lugar do resto. */
export function nomeDaWorktree(texto: string): string {
  return texto
    .trim()
    .toLowerCase()
    .normalize("NFD")
    .replace(/[̀-ͯ]/g, "")
    .replace(/[^a-z0-9_]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 48)
    .replace(/-+$/g, "");
}
