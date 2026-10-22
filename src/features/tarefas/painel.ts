/**
 * Fork (Tarefas): estado do painel por cima do app, do botão flutuante e da lista do dia flutuante.
 * Posições e escolhas ficam em `medza-tarefas-painel` (espelhado em `estado-da-interface.json`);
 * "aberto" não persiste: o app abre sempre com o painel fechado.
 */
import { useSyncExternalStore } from "react";

export const CHAVE_DO_PAINEL = "medza-tarefas-painel";

export interface Posicao { x: number; y: number }

interface EstadoDoPainel {
  aberto: boolean;
  iaVisivel: boolean;
  listaFixada: boolean;
  listaRecolhida: boolean;
  /** Canto superior esquerdo da lista do dia (px); null = encostada à direita. */
  posLista: Posicao | null;
  /** Distância do botão até o canto inferior direito (px). */
  posBotao: Posicao;
  botaoVisivel: boolean;
}

const PADRAO: EstadoDoPainel = { aberto: false, iaVisivel: true, listaFixada: false, listaRecolhida: false, posLista: null, posBotao: { x: 20, y: 64 }, botaoVisivel: true };

function posicao(v: unknown): Posicao | null {
  const p = v as Partial<Posicao> | null;
  return p && typeof p.x === "number" && typeof p.y === "number" && Number.isFinite(p.x) && Number.isFinite(p.y) ? { x: p.x, y: p.y } : null;
}

function ler(): EstadoDoPainel {
  try {
    const s = JSON.parse(localStorage.getItem(CHAVE_DO_PAINEL) ?? "null") as Partial<EstadoDoPainel> | null;
    if (!s || typeof s !== "object") return { ...PADRAO };
    return {
      aberto: false,
      iaVisivel: s.iaVisivel !== false,
      listaFixada: s.listaFixada === true,
      listaRecolhida: s.listaRecolhida === true,
      posLista: posicao(s.posLista),
      posBotao: posicao(s.posBotao) ?? PADRAO.posBotao,
      botaoVisivel: s.botaoVisivel !== false,
    };
  } catch {
    return { ...PADRAO };
  }
}

let estado = ler();
const ouvintes = new Set<() => void>();

export function mudarPainel(mudanca: Partial<EstadoDoPainel>) {
  estado = { ...estado, ...mudanca };
  const { aberto: _aberto, ...guardar } = estado;
  try { localStorage.setItem(CHAVE_DO_PAINEL, JSON.stringify(guardar)); } catch { /* sem localStorage */ }
  ouvintes.forEach((o) => o());
}

export function usePainel(): EstadoDoPainel {
  return useSyncExternalStore((o) => { ouvintes.add(o); return () => ouvintes.delete(o); }, () => estado);
}

export const abrirPainelDeTarefas = () => mudarPainel({ aberto: true });
export const fecharPainelDeTarefas = () => mudarPainel({ aberto: false });
export const alternarPainelDeTarefas = () => mudarPainel({ aberto: !estado.aberto });
export const alternarListaDoDia = () => mudarPainel({ listaFixada: !estado.listaFixada, listaRecolhida: false });

/** Mantém algo arrastável dentro da janela (a janela pode ter diminuído desde a última vez). */
export function dentroDaJanela(p: Posicao, largura: number, altura: number, margem = 8): Posicao {
  return {
    x: Math.min(Math.max(margem, p.x), Math.max(margem, window.innerWidth - largura - margem)),
    y: Math.min(Math.max(margem, p.y), Math.max(margem, window.innerHeight - altura - margem)),
  };
}

/** Só para testes. */
export function reiniciarPainelParaTeste() { estado = ler(); }
