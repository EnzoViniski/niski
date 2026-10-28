import { invoke } from "@tauri-apps/api/core";
import type { WorkspaceCollection } from "../types/workspace";
import { consertarColecao, consertarWorkspace, relatorioVazio } from "./consertarWorkspace";

/**
 * Cópias do JSON cru do workspace, guardadas **antes** de qualquer descarte ou conserto (e
 * antes de restaurar outra), para nada se perder em silêncio. Ficam no próprio localStorage (o
 * workspace é pequeno: alguns KB), só as últimas 3. Restaurar passa pelo mesmo conserto da leitura.
 */

export const CHAVE_DO_BACKUP = "omni-agents-workspaces.backup";
const LIMITE = 3;

export type MotivoDoBackup = "conserto" | "ilegivel" | "antes-de-restaurar";

export interface BackupDoWorkspace {
  /** ISO da hora em que a cópia foi guardada. */
  em: string;
  /** Chave de onde o JSON veio (`omni-agents-workspaces` ou a legada `omni-agents-workspace`). */
  chave: string;
  motivo: MotivoDoBackup;
  /** Contagens para a lista na tela, sem abrir o JSON. */
  workspaces: number;
  projetos: number;
  bruto: string;
}

export function listarBackups(): BackupDoWorkspace[] {
  try {
    const lidos: unknown = JSON.parse(localStorage.getItem(CHAVE_DO_BACKUP) ?? "[]");
    if (!Array.isArray(lidos)) return [];
    return lidos.filter(
      (item): item is BackupDoWorkspace =>
        Boolean(item) && typeof item === "object" && typeof item.em === "string" && typeof item.bruto === "string"
    );
  } catch {
    return [];
  }
}

/** Interpreta o JSON cru de qualquer das duas chaves como coleção, já consertada. */
export function colecaoDoBruto(bruto: string): WorkspaceCollection | null {
  let valor: unknown;
  try {
    valor = JSON.parse(bruto);
  } catch {
    return null;
  }
  const colecao = consertarColecao(valor, relatorioVazio());
  if (colecao) return colecao;
  // Chave legada: um workspace só.
  const workspace = valor && typeof valor === "object" && "projects" in valor ? consertarWorkspace(valor, relatorioVazio()) : null;
  return workspace ? { version: 1, workspaces: [workspace], activeWorkspaceId: workspace.id } : null;
}

function contar(bruto: string): { workspaces: number; projetos: number } {
  const colecao = colecaoDoBruto(bruto);
  if (!colecao) return { workspaces: 0, projetos: 0 };
  return {
    workspaces: colecao.workspaces.length,
    projetos: colecao.workspaces.reduce((total, workspace) => total + workspace.projects.length, 0),
  };
}

/** Guarda a cópia (sem repetir a mais recente idêntica). Falha de armazenamento não derruba nada. */
export function guardarBackup(chave: string, bruto: string, motivo: MotivoDoBackup, agora = new Date()): void {
  try {
    const atuais = listarBackups();
    if (atuais[0]?.bruto === bruto) return;
    const novo: BackupDoWorkspace = { em: agora.toISOString(), chave, motivo, ...contar(bruto), bruto };
    localStorage.setItem(CHAVE_DO_BACKUP, JSON.stringify([novo, ...atuais].slice(0, LIMITE)));
  } catch {
    /* sem espaço/armazenamento: segue sem a cópia */
  }
}

/** Evento de metadados no log em disco (`app.log`): só contagens e kinds, nunca caminho ou título. */
export function registrarEventoDoWorkspace(evento: string, detalhe: string): void {
  try {
    void Promise.resolve(invoke("registrar_evento_da_ui", { evento, detalhe })).catch(() => undefined);
  } catch {
    /* fora do Tauri (testes, navegador): sem log */
  }
}
