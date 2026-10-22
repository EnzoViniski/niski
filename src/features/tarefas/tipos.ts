/**
 * Fork (Tarefas): formato de cada tarefa (`<pasta>/tarefas/<id>.json`) e de cada pessoa
 * (`<pasta>/pessoas/<id>.json`). A pasta pode ser compartilhada (Drive, iCloud...) com outra pessoa
 * que também usa o Niski. Datas em ISO; dia em `AAAA-MM-DD` e hora em `HH:MM`, no fuso do PC.
 */

export const VERSAO_DA_TAREFA = 1;

export const STATUS = ["a_fazer", "fazendo", "revisao", "feito"] as const;
export type Status = (typeof STATUS)[number];
export const ROTULO_DO_STATUS: Record<Status, string> = { a_fazer: "A fazer", fazendo: "Fazendo", revisao: "Esperando", feito: "Feito" };
export const DICA_DO_STATUS: Record<Status, string> = {
  a_fazer: "Ainda não começou",
  fazendo: "Em andamento agora",
  revisao: "Depende de alguém ou de uma resposta",
  feito: "Concluída",
};

export const PRIORIDADES = ["baixa", "media", "alta", "urgente"] as const;
export type Prioridade = (typeof PRIORIDADES)[number];
export const ROTULO_DA_PRIORIDADE: Record<Prioridade, string> = { baixa: "Baixa", media: "Média", alta: "Alta", urgente: "Urgente" };

export interface Categoria { id: string; rotulo: string; cor: string; emoji: string }
export const CATEGORIAS: Categoria[] = [
  { id: "trabalho", rotulo: "Trabalho", cor: "#5b8def", emoji: "💼" },
  { id: "faculdade", rotulo: "Faculdade", cor: "#9b6bdf", emoji: "🎓" },
  { id: "saude", rotulo: "Saúde e médicos", cor: "#22a06b", emoji: "🩺" },
  { id: "casa", rotulo: "Casa", cor: "#e2a03f", emoji: "🏠" },
  { id: "financas", rotulo: "Finanças", cor: "#2fb3b3", emoji: "💰" },
  { id: "pessoal", rotulo: "Pessoal", cor: "#e0679a", emoji: "🌱" },
  { id: "outro", rotulo: "Outro", cor: "#7d8a99", emoji: "📌" },
];
export const categoria = (id: string) => CATEGORIAS.find((c) => c.id === id) ?? CATEGORIAS[CATEGORIAS.length - 1];

export const RECORRENCIAS = ["nunca", "diaria", "dias_uteis", "semanal", "mensal"] as const;
export type Recorrencia = (typeof RECORRENCIAS)[number];
export const ROTULO_DA_RECORRENCIA: Record<Recorrencia, string> = {
  nunca: "Não repete", diaria: "Todo dia", dias_uteis: "Dias úteis", semanal: "Toda semana", mensal: "Todo mês",
};

export interface ItemDoChecklist { id: string; texto: string; feito: boolean }
export interface Comentario { id: string; autor: string; texto: string; em: string }

export interface Tarefa {
  versao: number;
  id: string;
  titulo: string;
  descricao: string;
  status: Status;
  prioridade: Prioridade;
  categoria: string;
  etiquetas: string[];
  /** Id da pessoa (`Pessoa.id`); vazio = ninguém. */
  responsavel: string;
  criadaPor: string;
  /** `AAAA-MM-DD`; vazio = sem data. */
  data: string;
  /** `HH:MM`; vazio = dia todo. */
  hora: string;
  duracaoMin: number;
  local: string;
  recorrencia: Recorrencia;
  checklist: ItemDoChecklist[];
  /** Ids de outras tarefas. */
  relacionadas: string[];
  comentarios: Comentario[];
  /** Ordem dentro da coluna do quadro (menor em cima). */
  posicao: number;
  criadaEm: string;
  atualizadaEm: string;
  concluidaEm: string;
  /** Apagada fica gravada com a marca, para o apagar chegar na pasta da outra pessoa. */
  apagada: boolean;
}

export interface Pessoa {
  id: string;
  nome: string;
  cor: string;
  atualizadaEm: string;
}

export interface AjustesDasTarefas {
  /** Quem sou eu (gravado em `pessoas/<id>.json`). */
  eu: Pessoa;
  /** Pasta sincronizada escolhida; vazio = só neste computador. */
  pastaCompartilhada: string;
  visao: "dia" | "quadro" | "lista" | "semana";
  filtroPessoa: "todas" | "minhas" | string;
  filtroCategoria: string;
  ocultarFeitasAntigas: boolean;
}

export const CORES_DE_PESSOA = ["#5b8def", "#e0679a", "#22a06b", "#e2a03f", "#9b6bdf", "#2fb3b3"] as const;
