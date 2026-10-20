import type { AgenteSalvo } from "../features/agentes/agentesService";
import type { AgentCliStatus, Profile, TerminalSession } from "../features/terminal/terminalService";
import type { KanbanTask } from "../types/kanban";

// Só dados sintéticos: nenhum perfil, transcript ou CLI real é consultado.
export const clis: AgentCliStatus[] = [
  { id: "claude", label: "Claude", command: "claude", path: "/fake/claude", available: true, authenticated: true },
  { id: "codex", label: "Codex", command: "codex", path: "/fake/codex", available: true, authenticated: true },
  { id: "cursor", label: "Cursor", command: "agent", path: null, available: false, authenticated: false },
];
export const perfis: Profile[] = [
  { id: "pessoal", provider: "claude", name: "Pessoal", config_dir: "/fake/pessoal", builtin: true, authenticated: true, created_at_ms: 1, last_used_at_ms: null },
  { id: "trabalho", provider: "claude", name: "Trabalho", config_dir: "/fake/trabalho", builtin: false, authenticated: true, created_at_ms: 1, last_used_at_ms: null },
];
export const agente: AgenteSalvo = {
  name: "revisor", description: "Revê acessibilidade", cli: "claude", modelo: "", esforco: "", conta: "", ferramentas: ["Read"], saida: "veredito", instrucoes: "Confira o foco.", escopo: "global", caminho: "/fake/agents/revisor.md",
};
export const sessao: TerminalSession = {
  id: "sessao", project_id: "proj", name: "Kanban", cwd: "/fake/projeto", shell: "sh", state: "working", pid: null, output_seq: 0, rows: 30, cols: 120,
};
export const tarefa: KanbanTask = {
  id: "task", projectId: "proj", title: "Revisar login", description: "Confira o foco do formulário", status: "ready", position: 0, loopTotal: null, loopRemaining: null, resultNote: null, createdAt: "2026-10-19T12:00:00Z", updatedAt: "2026-10-19T12:00:00Z",
};
