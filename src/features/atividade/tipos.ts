export const TIPOS_DE_ATIVIDADE = ["precisa_de_voce", "terminou", "erro", "limite", "subagente_terminou", "agendamento"] as const;
export type TipoDeAtividade = typeof TIPOS_DE_ATIVIDADE[number];
export const ROTULOS_DA_ATIVIDADE: Record<TipoDeAtividade, string> = {
  agendamento: "Agendamento", // Execução da agenda (3D).
  precisa_de_voce: "Precisa de você", terminou: "Terminou o turno", erro: "Erro",
  limite: "Limite de uso", subagente_terminou: "Subagente terminou",
};
export interface ItemDeAtividade {
  id: string; conversa: string; sessao: string; projeto_id: string; projeto: string; titulo: string;
  plataforma: string; conta_id: string | null; conta: string; tipo: TipoDeAtividade;
  desde_ms: number; visto: boolean; resumo: string; orquestrador: string | null;
}
export type SomDoAviso = "Glass" | "Ping" | "Basso";
export interface PreferenciaDeAviso { quando: "sempre" | "fora_de_foco" | "nunca"; som: SomDoAviso | null }
export type PreferenciasDeAvisos = Record<TipoDeAtividade, PreferenciaDeAviso>;
export const AVISOS_PADRAO: PreferenciasDeAvisos = {
  agendamento: { quando: "fora_de_foco", som: "Ping" },
  precisa_de_voce: { quando: "fora_de_foco", som: "Glass" },
  terminou: { quando: "fora_de_foco", som: "Ping" },
  erro: { quando: "fora_de_foco", som: "Basso" },
  limite: { quando: "fora_de_foco", som: "Basso" },
  subagente_terminou: { quando: "fora_de_foco", som: "Ping" },
};

/** Configuração antiga ou incompleta continua válida ao ganhar tipos novos. */
export function preferenciasDeAvisos(salvas?: Partial<PreferenciasDeAvisos>): PreferenciasDeAvisos {
  return Object.fromEntries(TIPOS_DE_ATIVIDADE.map(tipo => {
    const salva = salvas?.[tipo];
    const padrao = AVISOS_PADRAO[tipo];
    return [tipo, {
      quando: ["sempre", "fora_de_foco", "nunca"].includes(salva?.quando ?? "") ? salva!.quando : padrao.quando,
      som: salva?.som === null || ["Glass", "Ping", "Basso"].includes(salva?.som ?? "") ? salva!.som : padrao.som,
    }];
  })) as PreferenciasDeAvisos;
}
