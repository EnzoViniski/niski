// Fork (Rotina): o que o engine devolve (`crates/omni-core/src/rotina/`). Campos em snake_case como no Rust.

export type EstadoDoItem = "feito" | "pulado" | "";

export interface ItemDoDia {
  id: string;
  tipo: "bloco" | "treino" | "tarefa" | "refeicao";
  titulo: string;
  detalhe: string;
  inicio: string;
  fim: string;
  area: string;
  cor: string;
  estado: EstadoDoItem;
  alvo: string;
  categoria: string;
}

export interface Meta {
  id: string; texto: string; area: string; alvo: number | null; progresso: number; unidade: string; concluida: boolean;
}

export interface Area { id: string; nome: string; cor: string; categoria: string; etiqueta: string }

export interface Dia {
  data: string;
  dia_da_semana: string;
  itens: ItemDoDia[];
  sem_hora: ItemDoDia[];
  cancelados: string[];
  agua: { ml: number; meta_ml: number; copo_ml: number };
  alimentacao: { kcal: number; proteina_g: number; refeicoes: number; agua_ml: number };
  treino: { ficha: { id: string; nome: string; titulo: string; exercicios: number } | null;
    sessao: { id: string; ficha: string; series: number; volume: number; concluida: boolean } | null };
  agora: string | null;
  proximo: string | null;
  foco: string;
  metas: Meta[];
}

export interface Bloco {
  id: string; titulo: string; area: string; tipo: string; dias: number[]; inicio: string; fim: string; data: string;
  lembrar: boolean; local: string; notas: string;
}

export interface SemanaDaRotina {
  segunda: string;
  hoje: string;
  dias: Dia[];
  foco: string;
  metas: Meta[];
  notas: string;
  planejada_em: string;
  imprevistos: { em: string; data: string; texto: string }[];
  areas: Area[];
  progresso: { itens_feitos: number; itens_total: number; treinos: number; meta_treinos: number; dias_com_agua_na_meta: number;
    metas_concluidas: number; metas_total: number };
  memoria: string;
  blocos_fixos: Bloco[];
}

export interface ExercicioDaFicha { nome: string; series: number; repeticoes: string; carga_kg: number | null; descanso_s: number; obs: string }
export interface Ficha { id: string; nome: string; titulo: string; exercicios: ExercicioDaFicha[]; dias: number[] }
export interface Serie { exercicio: string; carga_kg: number; repeticoes: number; obs: string; em: string }
export interface Sessao { id: string; data: string; ficha: string; inicio: string; fim: string; series: Serie[]; obs: string; concluida: boolean; volume?: number }
export interface Ponto { data: string; carga_max: number; volume: number; repeticoes_max: number; um_rm: number; series: number; recorde: boolean }

export interface Treino {
  hoje: string;
  fichas: Ficha[];
  meta_semanal: number;
  proxima_ficha: string | null;
  sessao_de_hoje: Sessao | null;
  sessoes: Sessao[];
  frequencia: { semana: string; treinos: number }[];
  exercicios: { nome: string; sessoes: number; ultima_data: string; carga_max: number; um_rm: number }[];
  evolucao?: { exercicio: string; pontos: Ponto[]; recorde_carga: number; recorde_um_rm: number; recorde_volume: number; ultima_vez: Serie[] };
}

export interface RefeicaoPlanejada { id: string; nome: string; hora: string; sugestao: string; kcal: number | null; proteina_g: number | null }
export interface Refeicao { id: string; hora: string; nome: string; texto: string; kcal: number | null; proteina_g: number | null; planejada: string }

export interface Alimentacao {
  data: string;
  plano: RefeicaoPlanejada[];
  meta_agua_ml: number;
  copo_ml: number;
  meta_kcal: number | null;
  meta_proteina_g: number | null;
  dia: { refeicoes: Refeicao[]; agua_ml: number };
  totais: { kcal: number; proteina_g: number; refeicoes: number; agua_ml: number };
  historico: { data: string; kcal: number; proteina_g: number; refeicoes: number; agua_ml: number }[];
}

export interface LembretePersonalizado { id: string; texto: string; hora: string; dias: number[]; data: string; tipo: string; ativo: boolean }
export interface Lembretes {
  ativo: boolean; antecedencia_min: number; tarefas: boolean; blocos: boolean; refeicoes: boolean; treino: boolean;
  agua: { ativo: boolean; intervalo_min: number }; acordar: string; dormir: string;
  planejar: { ativo: boolean; dia: number; hora: string };
  personalizados: LembretePersonalizado[];
}

export interface AjustesDaRotina {
  plataforma: string; conta: string; modelo: string; esforco: string; modo_conversa: "semanal" | "continua";
  telegram_lembretes: boolean; telegram_conversa: boolean;
}

export interface Aviso {
  chave: string; tipo: string; titulo: string; texto: string; em_ms: number; alvo: string; data: string; botoes: string[];
  enviado_ms: number; visto: boolean; resposta: string;
}
export interface Avisos { nao_vistos: number; avisos: Aviso[] }

export interface ConversaDaRotina { conversa: string | null; sessao: string | null; estado?: string; semana?: string }

export interface TelegramDaRotina {
  conectado: boolean; ativo: boolean; bot: string; erro: string | null; parear_url: string | null; expira_em_ms: number | null;
}

export const NOMES_CURTOS_DOS_DIAS = ["Seg", "Ter", "Qua", "Qui", "Sex", "Sáb", "Dom"];
