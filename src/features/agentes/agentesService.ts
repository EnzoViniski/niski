import { invoke } from "@tauri-apps/api/core";

export type CliDoAgente = "claude" | "codex" | "cursor";
export type SaidaDoAgente = "texto" | "plano" | "veredito" | "relatorio";
export type Escopo = "global" | "projeto";

/** Um modelo de agente: o `.md` em `~/.omni/agents/` ou `<projeto>/.omni/agents/`. */
export interface ModeloDeAgente {
  name: string;
  description: string;
  cli: CliDoAgente;
  modelo: string;
  /** Esforço de raciocínio (low…max no Claude). Vazio = padrão da CLI. */
  esforco: string;
  conta: string;
  ferramentas: string[];
  saida: SaidaDoAgente;
  instrucoes: string;
}

export interface AgenteSalvo extends ModeloDeAgente {
  escopo: Escopo;
  caminho: string;
}

export const FERRAMENTAS_COMUNS = ["Read", "Edit", "Write", "Bash", "Grep", "Glob", "WebFetch", "WebSearch"];

export const SAIDAS: { value: SaidaDoAgente; label: string; dica: string }[] = [
  { value: "texto", label: "Texto", dica: "Resposta livre: o que fez e como testou." },
  { value: "plano", label: "Plano", dica: "Divide o trabalho em partes (JSON) para outros agentes." },
  { value: "veredito", label: "Veredito", dica: "Aprova ou reprova, com a lista de problemas (JSON)." },
  { value: "relatorio", label: "Relatório", dica: "Resumo do que analisou ou documentou." },
];

export const modeloVazio = (): ModeloDeAgente => ({
  name: "", description: "", cli: "claude", modelo: "", esforco: "", conta: "", ferramentas: ["Read", "Grep", "Glob"], saida: "texto", instrucoes: "",
});

export function listarAgentes(projectPath: string | null) {
  return invoke<AgenteSalvo[]>("agentes_listar", { projectPath });
}

export function salvarAgente(escopo: Escopo, projectPath: string | null, modelo: ModeloDeAgente, nomeAnterior?: string) {
  return invoke<string>("agentes_salvar", { escopo, projectPath, modelo, nomeAnterior: nomeAnterior ?? null });
}

export function apagarAgente(escopo: Escopo, projectPath: string | null, nome: string) {
  return invoke<void>("agentes_apagar", { escopo, projectPath, nome });
}

export function restaurarKit() {
  return invoke<void>("agentes_restaurar_kit");
}

export function instalarNoClaudeCode(projectPath: string, modelo: ModeloDeAgente) {
  return invoke<string>("agentes_instalar_claude", { projectPath, modelo });
}

/** Leva uns 30–60 s: o Claude (opus, esforço alto) escreve o modelo a partir da frase. */
export function criarComIa(descricao: string, cli: CliDoAgente, projectPath: string | null, conta?: string) {
  return invoke<ModeloDeAgente>("agentes_criar_com_ia", { descricao, cli, conta: conta || null, projectPath });
}

export interface ModeloDaCli { id: string; label: string; esforcos: string[] }
/** Modelos e esforços de cada CLI (os do Codex vêm da lista que a própria CLI baixou). */
export function listarModelos() {
  return invoke<Record<CliDoAgente, ModeloDaCli[]>>("agentes_modelos");
}

export const ROTULO_ESFORCO: Record<string, string> = { low: "Baixo", medium: "Médio", high: "Alto", xhigh: "Muito alto", max: "Máximo", minimal: "Mínimo" };

/** "Revisor de acessibilidade" → "revisor-de-acessibilidade" (o nome vira nome de arquivo). */
export function nomeDeArquivo(texto: string): string {
  return texto.normalize("NFD").replace(/[\u0300-\u036f]/g, "").toLowerCase()
    .replace(/[^a-z0-9]+/g, "-").replace(/^-+|-+$/g, "").slice(0, 40).replace(/-+$/g, "");
}
