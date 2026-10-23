/**
 * Fork (troca-de-contexto): quando mostrar o cartão "onde parei" e até onde vai a marca "novo desde a
 * sua última olhada".
 *
 * "Olhando" = a conversa está à vista e é a que tem a sua atenção: no PC, o painel em foco com a
 * Conversa na tela (`OlhandoContext`, posto pela aba de agente); no celular e na web, a conversa
 * aberta. Em ambos, com a janela visível.
 *
 * - Ao começar a olhar (focar o painel, clicar na aba ou na lateral, voltar ao app), guarda a olhada
 *   anterior: é a fronteira desta volta. O cartão abre se `deveMostrar` (voltou depois de um tempo e
 *   chegou coisa nova) ou se o atalho pediu (`pedirOndeParei`).
 * - Enquanto olha, a olhada anda junto com a timeline; ao parar de olhar, grava a hora.
 * - Fecha ao digitar no campo ou com Esc; reabre pelo botão "Onde parei".
 *
 * Nada aqui lê o engine: tudo vem da timeline que a conversa já lê (com ETag).
 */
import { createContext, useCallback, useContext, useEffect, useRef, useState, type RefObject } from "react";
import type { Conversation, Mensagem, Tarefa } from "../../mobile/api";
import { deveMostrar, linhaEssencial, montarOndeParei, ultimoIndice, type OndeParei } from "./ondeParei";
import { lerOlhada, marcarOlhada, type Olhada } from "./ultimaOlhada";
import { consumirPedido, EVENTO_ABRIR_ONDE_PAREI, publicarResumo } from "./painelDaTroca";

/** A conversa tem a sua atenção agora. Fora da aba de agente (celular, web): sim. */
export const OlhandoContext = createContext(true);

/** Enquanto você olha, a hora da olhada é renovada neste ritmo (para o app fechado no meio contar). */
const RENOVAR_OLHADA_MS = 60_000;

/** A janela está à vista e com o foco (outro app na frente não conta como olhar). */
function useJanelaComVoce(): boolean {
  const ler = () => typeof document === "undefined"
    || (document.visibilityState !== "hidden" && (typeof document.hasFocus !== "function" || document.hasFocus()));
  const [comVoce, setComVoce] = useState(ler);
  useEffect(() => {
    const aoMudar = () => setComVoce(ler());
    // O `blur` chega antes de o foco ir para outro lugar da própria página: confere no quadro seguinte.
    const aoSair = () => window.setTimeout(aoMudar, 0);
    document.addEventListener("visibilitychange", aoMudar);
    window.addEventListener("focus", aoMudar);
    window.addEventListener("blur", aoSair);
    return () => {
      document.removeEventListener("visibilitychange", aoMudar);
      window.removeEventListener("focus", aoMudar);
      window.removeEventListener("blur", aoSair);
    };
  }, []);
  return comVoce;
}

export interface OndePareiNaTela {
  cartao: OndeParei;
  aberto: boolean;
  /** Número da primeira mensagem nova desta volta (a marca na conversa), ou `null`. */
  marca: number | null;
  /** Quando foi a olhada anterior (para "você saiu há 12 min"). */
  olhadaEm: number | null;
  /** Há o que mostrar se reabrir. */
  podeReabrir: boolean;
  abrir: () => void;
  fechar: () => void;
}

interface Volta { anterior: Olhada | null; avaliada: boolean; forcar: boolean }

export function useOndeParei({ conversa, mensagens, tarefas, carregado, tela }: {
  conversa: Conversation;
  mensagens: Mensagem[];
  tarefas?: Tarefa[] | null;
  /** A timeline desta conversa já chegou (antes disso nada é gravado nem avaliado). */
  carregado: boolean;
  tela: RefObject<HTMLElement | null>;
}): OndePareiNaTela {
  const comAtencao = useContext(OlhandoContext);
  const visivel = useJanelaComVoce();
  const olhando = comAtencao && visivel;
  const id = conversa.id;
  const [volta, setVolta] = useState<Volta | null>(null);
  const [aberto, setAberto] = useState(false);

  // Sem você olhando, vale a olhada gravada (é dela que contam as novidades para a lateral).
  const base = olhando ? volta?.anterior ?? null : lerOlhada(id);
  const cartao = montarOndeParei({ mensagens, tarefas, conversa, olhada: base, agora: Date.now(), desde: base?.n ?? null });
  const n = ultimoIndice(mensagens);
  const tom = cartao.estado.tom;

  const vivo = useRef({ n, tom, carregado });
  vivo.current = { n, tom, carregado };
  const olhandoRef = useRef(olhando);
  olhandoRef.current = olhando;

  // 1. Começou a olhar: a olhada anterior vira a fronteira desta volta. Parou: grava a hora.
  useEffect(() => {
    if (!olhando) return;
    setVolta({ anterior: lerOlhada(id), avaliada: false, forcar: consumirPedido(id) });
    setAberto(false);
    return () => {
      const atual = vivo.current;
      if (atual.carregado && atual.n >= 0) marcarOlhada(id, { n: atual.n, em: Date.now(), tom: atual.tom });
    };
  }, [olhando, id]);

  // 2. Com a timeline na mão, decide se o cartão abre nesta volta.
  useEffect(() => {
    if (!olhando || !carregado || !volta || volta.avaliada) return;
    const naVolta = montarOndeParei({ mensagens, tarefas, conversa, olhada: volta.anterior, agora: Date.now(), desde: volta.anterior?.n ?? null });
    setAberto(volta.forcar || deveMostrar(naVolta, volta.anterior));
    setVolta({ ...volta, avaliada: true });
  }, [olhando, carregado, volta, mensagens, tarefas, conversa]);

  // 3. Enquanto olha, a olhada acompanha a timeline (e a hora é renovada de tempos em tempos).
  useEffect(() => {
    if (!olhando || !carregado || n < 0) return;
    const gravar = () => marcarOlhada(id, { n: vivo.current.n, em: Date.now(), tom: vivo.current.tom });
    gravar();
    const relogio = window.setInterval(gravar, RENOVAR_OLHADA_MS);
    return () => window.clearInterval(relogio);
  }, [olhando, carregado, id, n, tom]);

  // 4. O atalho pediu o cartão desta conversa, e ela já está à vista.
  useEffect(() => {
    const aoPedir = (evento: Event) => {
      if ((evento as CustomEvent<string>).detail !== id || !olhandoRef.current) return;
      consumirPedido(id);
      setAberto(true);
    };
    window.addEventListener(EVENTO_ABRIR_ONDE_PAREI, aoPedir);
    return () => window.removeEventListener(EVENTO_ABRIR_ONDE_PAREI, aoPedir);
  }, [id]);

  // 5. Some ao interagir: digitar no campo de mensagem ou Esc dentro da conversa.
  useEffect(() => {
    const alvo = tela.current;
    if (!alvo || !aberto) return;
    const aoTeclar = (evento: KeyboardEvent) => {
      if (evento.key === "Escape") { setAberto(false); return; }
      const de = evento.target as HTMLElement | null;
      if (de?.closest("textarea, input, [contenteditable]") && !evento.metaKey && !evento.ctrlKey && evento.key.length === 1) setAberto(false);
    };
    alvo.addEventListener("keydown", aoTeclar);
    return () => alvo.removeEventListener("keydown", aoTeclar);
  }, [tela, aberto]);

  // 6. A lateral ("Na tela") lê daqui o essencial desta conversa.
  const linha = linhaEssencial(cartao.estado, cartao);
  const novas = olhando ? 0 : cartao.novas;
  useEffect(() => {
    if (!carregado) return;
    publicarResumo(id, { tom, linha, novas });
  }, [id, carregado, tom, linha, novas]);
  useEffect(() => () => publicarResumo(id, null), [id]);

  const abrir = useCallback(() => setAberto(true), []);
  const fechar = useCallback(() => setAberto(false), []);
  return {
    cartao,
    aberto: aberto && olhando,
    marca: olhando && volta?.avaliada && cartao.novas > 0 ? cartao.primeiroNovo : null,
    olhadaEm: base?.em ?? null,
    podeReabrir: Boolean(cartao.pedido || cartao.novas > 0),
    abrir,
    fechar,
  };
}
