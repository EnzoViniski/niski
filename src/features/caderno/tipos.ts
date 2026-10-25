/**
 * Fork (Caderno): formato dos arquivos de cada caderno (`src-tauri/src/cadernos.rs`). Tudo que vem
 * do disco passa por `normalizarMeta` e `lerConversa`: arquivo estranho vira padrão, nunca quebra a tela.
 */
import type { IaEscolhida } from "../roadmaps/tipos";

/** Parte dos Roadmaps que está na tela. */
export type SecaoDosRoadmaps = "roadmaps" | "caderno";

export const VERSAO_DO_CADERNO = 1;

export type TipoDeCaderno = "livro" | "estudo" | "vida" | "livre";
export type Papel = "pautado" | "quadriculado" | "pontilhado" | "liso";
export type Letra = "manuscrita" | "serifa" | "normal";

export const SOBRE_O_TIPO: Record<TipoDeCaderno, { nome: string; emoji: string; dica: string }> = {
  livro: { nome: "Livro", emoji: "📖", dica: "Anotações de um livro, capítulo por capítulo." },
  estudo: { nome: "Estudo", emoji: "🎓", dica: "Matéria, curso, assunto que você está aprendendo." },
  vida: { nome: "Vida", emoji: "🌱", dica: "Diário, reflexões, metas, ideias." },
  livre: { nome: "Livre", emoji: "✏️", dica: "Qualquer coisa." },
};

/** Capas: cor da capa e da lombada. */
export const CAPAS = ["#2f4b7c", "#7a2e2e", "#2f6b4f", "#8a6a2b", "#5b3b78", "#1f5f6b", "#9c4a6b", "#3d3d3d"] as const;

export interface PaginaDoCaderno { id: string; titulo: string; criadaEm: string; atualizadaEm: string }

export interface MetaDoCaderno {
  versao: number;
  id: string;
  nome: string;
  tipo: TipoDeCaderno;
  /** Só para livro. */
  autor: string;
  capa: string;
  papel: Papel;
  letra: Letra;
  ia: IaEscolhida;
  paginas: PaginaDoCaderno[];
  criadoEm: string;
  atualizadoEm: string;
  arquivado: boolean;
}

export interface MensagemDoCaderno { de: "eu" | "ia"; texto: string; em: string; paginaId?: string }

export const ID_VALIDO = /^[a-z0-9][a-z0-9-]{0,63}$/;

export function novoId(prefixo: string, agora = Date.now()): string {
  return `${prefixo}-${agora.toString(36)}-${Math.random().toString(36).slice(2, 7)}`;
}

const texto = (v: unknown, max: number, padrao = "") => (typeof v === "string" ? v.slice(0, max) : padrao);
const escolha = <T extends string>(v: unknown, opcoes: readonly T[], padrao: T): T => (opcoes.includes(v as T) ? (v as T) : padrao);

export function normalizarMeta(bruto: unknown): MetaDoCaderno | null {
  const m = bruto as Partial<MetaDoCaderno> | null;
  if (!m || typeof m !== "object" || typeof m.id !== "string" || !ID_VALIDO.test(m.id)) return null;
  const agora = new Date().toISOString();
  const ia = m.ia && typeof m.ia === "object" && (m.ia.cli === "claude" || m.ia.cli === "codex")
    ? { cli: m.ia.cli, ...(typeof m.ia.conta === "string" && m.ia.conta ? { conta: m.ia.conta } : {}), ...(typeof m.ia.modelo === "string" && m.ia.modelo ? { modelo: m.ia.modelo } : {}) }
    : { cli: "claude" as const };
  const vistas = new Set<string>();
  const paginas = (Array.isArray(m.paginas) ? m.paginas : []).flatMap((p) => {
    const pg = p as Partial<PaginaDoCaderno>;
    if (typeof pg?.id !== "string" || !ID_VALIDO.test(pg.id) || vistas.has(pg.id)) return [];
    vistas.add(pg.id);
    return [{ id: pg.id, titulo: texto(pg.titulo, 200), criadaEm: texto(pg.criadaEm, 40, agora), atualizadaEm: texto(pg.atualizadaEm, 40, agora) }];
  });
  return {
    versao: typeof m.versao === "number" ? m.versao : VERSAO_DO_CADERNO,
    id: m.id,
    nome: texto(m.nome, 120) || "Caderno sem nome",
    tipo: escolha(m.tipo, ["livro", "estudo", "vida", "livre"] as const, "livre"),
    autor: texto(m.autor, 120),
    capa: typeof m.capa === "string" && /^#[0-9a-f]{6}$/i.test(m.capa) ? m.capa : CAPAS[0],
    papel: escolha(m.papel, ["pautado", "quadriculado", "pontilhado", "liso"] as const, "pautado"),
    letra: escolha(m.letra, ["manuscrita", "serifa", "normal"] as const, "manuscrita"),
    ia,
    paginas,
    criadoEm: texto(m.criadoEm, 40, agora),
    atualizadoEm: texto(m.atualizadoEm, 40, agora),
    arquivado: m.arquivado === true,
  };
}

export function lerConversa(textoDoArquivo: string | null): MensagemDoCaderno[] {
  return (textoDoArquivo ?? "").split("\n").flatMap((linha) => {
    if (!linha.trim()) return [];
    try {
      const m = JSON.parse(linha) as Partial<MensagemDoCaderno>;
      if ((m.de !== "eu" && m.de !== "ia") || typeof m.texto !== "string") return [];
      return [{ de: m.de, texto: m.texto, em: typeof m.em === "string" ? m.em : "", ...(typeof m.paginaId === "string" ? { paginaId: m.paginaId } : {}) }];
    } catch { return []; }
  });
}
