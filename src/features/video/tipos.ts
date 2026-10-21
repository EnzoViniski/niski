// Fork (Editor de vídeo): formatos dos arquivos do projeto (ver `src-tauri/src/videos/projetos.rs`).

export type CliDoVideo = "claude" | "codex";

export interface IaDoVideo {
  cli: CliDoVideo;
  /** Id do perfil (conta). Sem ele, a primeira conta com login. */
  conta?: string;
  modelo?: string;
}

export type Formato = "16:9" | "9:16" | "1:1" | "4:5";

export interface Briefing {
  descricao: string;
  estilo: string;
  duracao: number;
  formato: Formato;
  publico: string;
  plataforma: string;
  observacoes: string;
}

/** `projeto.json`. */
export interface Projeto {
  id: string;
  versao: number;
  nome: string;
  formato: Formato;
  largura: number;
  altura: number;
  fps: number;
  cor: string;
  ia: IaDoVideo;
  briefing: Briefing;
  pastasDeReferencia: string[];
  /** Sessões da CLI por conta (`claude:<perfil>` → id da sessão), para o chat continuar de onde parou. */
  sessoes?: Record<string, string>;
  hyperframes: string;
  criadoEm: number;
  atualizadoEm: number;
}

export type TipoDeMidia = "video" | "imagem" | "audio" | "fonte" | "animacao" | "texto" | "outro";
export type Papel = "usar" | "referencia";

export interface Midia {
  id: string;
  nome: string;
  arquivo: string;
  tipo: TipoDeMidia;
  papel: Papel;
  hash: string;
  tamanho: number;
  nota: string;
  adicionadoEm: number;
}

export interface LinkDeReferencia { url: string; nota: string }

/** `midias.json`. */
export interface Midias { versao: number; itens: Midia[]; links: LinkDeReferencia[] }

export interface PastaDeReferencia {
  versao: number;
  id: string;
  nome: string;
  descricao: string;
  cor: string;
  itens: Midia[];
  links: LinkDeReferencia[];
  criadoEm: number;
  atualizadoEm: number;
}

export interface ItemDaLista { id: string; projeto: Projeto }

export interface VersaoDoHistorico { n: number; motivo: string; quando: number; arquivos: string[] }

export interface MidiaFaltando { id: string; nome: string; arquivo: string; motivo: "sumiu" | "mudou" }

/** Uma linha do `conversa.jsonl`. */
export interface MensagemDoChat {
  de: "eu" | "ia" | "sistema";
  texto: string;
  quando: number;
  /** Quem escreveu, numa sessão a dois. */
  autor?: string;
  cli?: CliDoVideo;
  modelo?: string;
  /** Arquivos que o agente mexeu nesta resposta. */
  arquivos?: string[];
  erro?: boolean;
}
