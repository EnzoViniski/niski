// Fork (Onda 2.0): a timeline da conversa — páginas, agrupamento e rolagem — saiu do `MobileApp.tsx`
// sem mudança de lógica. Os hooks guardam o mesmo estado e os mesmos efeitos, na mesma ordem.
import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState, type Dispatch, type MutableRefObject, type RefObject, type SetStateAction } from "react";
import { request, type Mensagem, type Timeline } from "../api";

/** Ritmo do polling com o agente ativo. 5 s faz a conversa parecer travada; 1,2 s parece ao vivo.
 *  Barato: com o `ETag`, consulta sem novidade volta 304 sem corpo. */
export const POLL_ATIVO_MS = 1_200;
/** Parado, ainda curto: uma resposta digitada no PC aparece aqui em poucos segundos. */
export const POLL_PARADO_MS = 3_000;

/** Distância do fim (px) que ainda conta como "lendo o fim": a tela acompanha o que chega. */
const PERTO_DO_FIM_PX = 120;

/** O servidor numera as mensagens (`<conversa>:<n>`) na ordem do transcript. */
export function indiceDaMensagem(id: string): number {
  const n = Number(id.slice(id.lastIndexOf(":") + 1));
  return Number.isFinite(n) ? n : -1;
}

/** Junta páginas da conversa pela numeração do servidor: sem repetir e na ordem do transcript. */
export function juntarPaginas(...paginas: Mensagem[][]): Mensagem[] {
  const porIndice = new Map<number, Mensagem>();
  for (const pagina of paginas) for (const mensagem of pagina) porIndice.set(indiceDaMensagem(mensagem.id), mensagem);
  return [...porIndice.entries()].sort((a, b) => a[0] - b[0]).map(([, mensagem]) => mensagem);
}

/** Distância do topo (px) em que rolar para cima já busca as mensagens anteriores. */
const PERTO_DO_TOPO_PX = 160;

/** O que a conversa desenha, já agrupado: ações seguidas viram um bloco só. */
export type Bloco =
  | { tipo: "fala"; mensagem: Mensagem }
  | { tipo: "acoes"; id: string; acoes: Mensagem[] }
  | { tipo: "comando"; mensagem: Mensagem }
  | { tipo: "aviso"; mensagem: Mensagem }
  // Fork: plano do plan mode (`ExitPlanMode`), em cartão próprio.
  | { tipo: "plano"; mensagem: Mensagem };

export function agruparMensagens(mensagens: Mensagem[]): Bloco[] {
  const blocos: Bloco[] = [];
  for (const mensagem of mensagens) {
    const tipo = mensagem.kind ?? "text";
    if (tipo === "tool" && mensagem.plano) blocos.push({ tipo: "plano", mensagem });
    else if (tipo === "tool") {
      const anterior = blocos[blocos.length - 1];
      if (anterior?.tipo === "acoes") anterior.acoes.push(mensagem);
      else blocos.push({ tipo: "acoes", id: mensagem.id, acoes: [mensagem] });
    } else if (tipo === "command") blocos.push({ tipo: "comando", mensagem });
    else if (tipo === "notice") blocos.push({ tipo: "aviso", mensagem });
    else blocos.push({ tipo: "fala", mensagem });
  }
  return blocos;
}

/** Quem rola a conversa: a lista de mensagens (web) ou a página inteira (celular). */
function rolagemDe(elemento: HTMLElement | null): HTMLElement | null {
  for (let atual = elemento?.parentElement ?? null; atual; atual = atual.parentElement) {
    const { overflowY } = getComputedStyle(atual);
    if (overflowY === "auto" || overflowY === "scroll") return atual;
  }
  return (document.scrollingElement as HTMLElement | null) ?? null;
}

function pertoDoFim(rolagem: HTMLElement | null): boolean {
  if (!rolagem) return true;
  return rolagem.scrollHeight - rolagem.scrollTop - rolagem.clientHeight < PERTO_DO_FIM_PX;
}

/** As mensagens da página ao vivo mais as antigas que quem rola para cima foi buscando. */
export function useMensagensAntigas({ data, base, fim, setError }: {
  data: Timeline | null;
  /** `/conversas/<id>/timeline`. */
  base: string;
  /** O fim da lista: é por ele que se acha quem rola a conversa. */
  fim: RefObject<HTMLDivElement | null>;
  setError: Dispatch<SetStateAction<string | null>>;
}) {
  // Páginas antigas buscadas por quem rola para cima. Ficam acima das recentes, na mesma rolagem:
  // antes "Mensagens anteriores" trocava a tela por outra página e era preciso paginar de volta.
  const [antigas, setAntigas] = useState<Mensagem[]>([]);
  // `undefined` = ainda não buscou nenhuma (vale o `prev_cursor` da página ao vivo); `null` = chegou ao início.
  const [cursorAntigas, setCursorAntigas] = useState<number | null | undefined>(undefined);
  const [buscandoAntigas, setBuscandoAntigas] = useState(false);
  const buscandoAntigasRef = useRef(false);
  // Altura da conversa antes de pôr as antigas em cima: a tela fica onde quem lia estava.
  const ancora = useRef<{ altura: number; topo: number } | null>(null);

  const recentes = useMemo(() => data?.timeline.messages ?? [], [data]);
  const mensagens = useMemo(() => antigas.length ? juntarPaginas(antigas, recentes) : recentes, [antigas, recentes]);
  // Com antigas na tela, o que sai da janela ao vivo (a conversa andou) entra nelas: não abre buraco.
  useEffect(() => {
    if (antigas.length && recentes.length) setAntigas(atual => juntarPaginas(atual, recentes));
  }, [recentes]); // eslint-disable-line react-hooks/exhaustive-deps
  const anteriorA = antigas.length ? cursorAntigas ?? null : data?.timeline.prev_cursor ?? null;

  const buscarAntigas = useCallback(async () => {
    if (anteriorA === null || buscandoAntigasRef.current) return;
    buscandoAntigasRef.current = true;
    setBuscandoAntigas(true);
    try {
      const pagina = await request<Timeline>(`${base}?cursor=${anteriorA}`);
      const rolagem = rolagemDe(fim.current);
      if (rolagem) ancora.current = { altura: rolagem.scrollHeight, topo: rolagem.scrollTop };
      setAntigas(atual => juntarPaginas(pagina.timeline.messages, atual, recentes));
      setCursorAntigas(pagina.timeline.prev_cursor ?? null);
    } catch (motivo) {
      setError(String(motivo));
    } finally {
      buscandoAntigasRef.current = false;
      setBuscandoAntigas(false);
    }
  }, [anteriorA, base, recentes]);
  const buscarAntigasRef = useRef(buscarAntigas);
  buscarAntigasRef.current = buscarAntigas;

  // As antigas entraram em cima: compensa a altura nova para o que estava na tela não sair do lugar.
  useLayoutEffect(() => {
    const antes = ancora.current;
    const rolagem = rolagemDe(fim.current);
    if (!antes || !rolagem) return;
    ancora.current = null;
    rolagem.scrollTop = antes.topo + (rolagem.scrollHeight - antes.altura);
  }, [antigas]);

  return { mensagens, anteriorA, buscandoAntigas, buscarAntigas, buscarAntigasRef };
}

/** Blocos da conversa, os separadores de dia e quais falas do agente seguem coladas na anterior. */
export function useBlocos(mensagens: Mensagem[]) {
  const blocos = useMemo(() => agruparMensagens(mensagens), [mensagens]);
  // Só a primeira fala do agente em cada turno leva "Claude · 17:54"; as seguintes (falas curtas
  // entre ações) seguem coladas, sem repetir o cabeçalho.
  // "Hoje", "Ontem" ou a data entre mensagens de dias diferentes — numa conversa longa é o que dá
  // noção de tempo sem olhar hora por hora.
  const separadores = useMemo(() => {
    const mapa = new Map<number, string>();
    let anterior = "";
    blocos.forEach((bloco, indice) => {
      const quando = bloco.tipo === "acoes" ? bloco.acoes[0]?.timestamp : bloco.mensagem.timestamp;
      const data = quando ? new Date(quando) : null;
      if (!data || Number.isNaN(data.getTime())) return;
      const chave = data.toDateString();
      if (chave === anterior) return;
      anterior = chave;
      const hoje = new Date();
      const ontem = new Date(hoje.getTime() - 86_400_000);
      mapa.set(indice, chave === hoje.toDateString() ? "Hoje" : chave === ontem.toDateString() ? "Ontem"
        : data.toLocaleDateString("pt-BR", { day: "2-digit", month: "long", year: data.getFullYear() === hoje.getFullYear() ? undefined : "numeric" }));
    });
    return mapa;
  }, [blocos]);

  const continuacoes = useMemo(() => {
    const indices = new Set<number>();
    let jaFalou = false;
    blocos.forEach((bloco, indice) => {
      if (bloco.tipo !== "fala") return;
      if (bloco.mensagem.role === "user") { jaFalou = false; return; }
      if (jaFalou) indices.add(indice);
      jaFalou = true;
    });
    return indices;
  }, [blocos]);

  return { blocos, separadores, continuacoes };
}

/** Acompanha o fim enquanto quem lê está nele; lendo mais acima, avisa "novas mensagens" em vez de
 *  pular. Chegar ao topo busca as anteriores. */
export function useRolagem({ fim, carregado, buscarAntigasRef, ultimaId, ativo }: {
  fim: RefObject<HTMLDivElement | null>;
  /** Já chegou a primeira timeline (antes disso não há o que rolar). */
  carregado: boolean;
  buscarAntigasRef: MutableRefObject<() => Promise<void>>;
  ultimaId: string;
  ativo: boolean;
}) {
  const [novidades, setNovidades] = useState(false);
  const grudado = useRef(true);

  // Saber se quem lê está no fim. Lendo mensagens antigas, a tela não pode pular sozinha para baixo
  // a cada novidade — antes pulava, e era impossível ler o começo de uma resposta longa.
  useEffect(() => {
    const rolagem = rolagemDe(fim.current);
    const alvo: HTMLElement | Window | null = rolagem === document.scrollingElement ? window : rolagem;
    if (!alvo) return;
    const aoRolar = () => {
      grudado.current = pertoDoFim(rolagem);
      if (grudado.current) setNovidades(false);
      // Chegou ao topo: as anteriores entram sozinhas, como num chat.
      else if (rolagem && rolagem.scrollTop < PERTO_DO_TOPO_PX) void buscarAntigasRef.current();
    };
    alvo.addEventListener("scroll", aoRolar, { passive: true });
    return () => alvo.removeEventListener("scroll", aoRolar);
  }, [carregado]);

  const irParaOFim = useCallback((suave = false) => {
    fim.current?.scrollIntoView?.({ block: "end", behavior: suave ? "smooth" : "auto" });
    grudado.current = true;
    setNovidades(false);
  }, []);

  // Chegou coisa nova: acompanha se estava no fim; senão avisa com o botão "novas mensagens".
  useEffect(() => {
    if (grudado.current) irParaOFim();
    else if (ultimaId) setNovidades(true);
  }, [ultimaId, ativo, irParaOFim]);

  return { grudado, novidades, irParaOFim };
}
