// Fork: Jarvis. Estado da janela do Jarvis montado a partir dos eventos `jarvis` do Rust
// (`src-tauri/src/jarvis/mod.rs`, `ParaATela`). Função pura, testada sem Tauri.

export type EstadoDaVoz = "iniciando" | "ouvindo" | "pensando" | "falando" | "pausado";

export type EventoDoJarvis =
  | { tipo: "estado"; estado: EstadoDaVoz }
  | { tipo: "parcial"; texto: string }
  | { tipo: "voce_disse"; texto: string }
  | { tipo: "resposta"; texto: string }
  | { tipo: "resposta_fim" }
  | { tipo: "nivel"; mic: number; fala: number }
  | { tipo: "interrompido" }
  | { tipo: "ferramenta"; nome: string; estado: string }
  | { tipo: "vozes"; vozes: { id: string; nome: string; idioma: string; genero: string; qualidade: string }[]; voz: string | null }
  | { tipo: "aviso"; msg: string }
  | { tipo: "erro"; msg: string };

export interface Fala {
  id: number;
  quem: "voce" | "jarvis";
  texto: string;
  /** Resposta ainda chegando. */
  aberta?: boolean;
  /** Resposta cortada por uma interrupção. */
  cortada?: boolean;
}

export interface EstadoDoJarvis {
  estado: EstadoDaVoz;
  falas: Fala[];
  parcial: string;
  aviso: string | null;
  erro: string | null;
  proximoId: number;
  ferramenta: { nome: string; estado: string } | null;
}

/** Só as últimas falas: a janela é um painel de conversa, não um histórico. */
const MAXIMO_DE_FALAS = 40;

export const estadoInicial: EstadoDoJarvis = { estado: "iniciando", falas: [], parcial: "", aviso: null, erro: null, proximoId: 1, ferramenta: null };

function fecharResposta(falas: Fala[], cortada: boolean): Fala[] {
  const ultima = falas[falas.length - 1];
  if (!ultima?.aberta) return falas;
  return [...falas.slice(0, -1), { ...ultima, aberta: false, cortada: cortada || undefined }];
}

function acrescentar(estado: EstadoDoJarvis, fala: Omit<Fala, "id">): EstadoDoJarvis {
  const falas = [...estado.falas, { ...fala, id: estado.proximoId }].slice(-MAXIMO_DE_FALAS);
  return { ...estado, falas, proximoId: estado.proximoId + 1 };
}

export function aplicarEvento(estado: EstadoDoJarvis, evento: EventoDoJarvis | { tipo: "reiniciar" }): EstadoDoJarvis {
  switch (evento.tipo) {
    case "reiniciar":
      return { ...estado, estado: "iniciando", erro: null, aviso: null, parcial: "", ferramenta: null };
    case "estado":
      return { ...estado, estado: evento.estado };
    case "parcial":
      return { ...estado, parcial: evento.texto };
    case "voce_disse":
      return acrescentar({ ...estado, falas: fecharResposta(estado.falas, true), parcial: "", erro: null, ferramenta: null }, { quem: "voce", texto: evento.texto });
    case "ferramenta":
      return { ...estado, ferramenta: { nome: evento.nome, estado: evento.estado } };
    case "resposta": {
      const ultima = estado.falas[estado.falas.length - 1];
      if (ultima?.quem === "jarvis" && ultima.aberta) {
        return { ...estado, falas: [...estado.falas.slice(0, -1), { ...ultima, texto: ultima.texto + evento.texto }] };
      }
      return acrescentar(estado, { quem: "jarvis", texto: evento.texto, aberta: true });
    }
    case "resposta_fim":
      return { ...estado, falas: fecharResposta(estado.falas, false) };
    case "interrompido":
      return { ...estado, falas: fecharResposta(estado.falas, true) };
    case "aviso":
      return { ...estado, aviso: evento.msg };
    case "erro":
      return { ...estado, erro: evento.msg };
    case "nivel":
    case "vozes":
      return estado;
  }
}

export const ROTULO_DO_ESTADO: Record<EstadoDaVoz, string> = {
  iniciando: "Ligando…",
  ouvindo: "Ouvindo",
  pensando: "Pensando…",
  falando: "Falando",
  pausado: "Microfone pausado",
};
