import { useEffect, useSyncExternalStore } from "react";
import { request, requestSeMudou } from "../../mobile/api";
import { usePoll } from "../../mobile/usePoll";
import type { ItemDeAtividade } from "./tipos";

interface Estado { itens: ItemDeAtividade[]; carregado: boolean; erro: string | null }
let estado: Estado = { itens: [], carregado: false, erro: null };
const ouvintes = new Set<() => void>();
let emVoo: Promise<void> | null = null;
let ultimaLeitura = 0;
let geracao = 0;
function publicar(novo: Estado) { estado = novo; ouvintes.forEach(ouvir => ouvir()); }
function assinar(ouvir: () => void) { ouvintes.add(ouvir); return () => { ouvintes.delete(ouvir); }; }
function falhou(motivo: unknown) {
  const erro = motivo instanceof Error ? motivo.message : String(motivo);
  if (estado.erro !== erro) publicar({ ...estado, erro });
}
function assinarVisibilidade(ouvir: () => void) { document.addEventListener("visibilitychange", ouvir); return () => document.removeEventListener("visibilitychange", ouvir); }
function estaVisivel() { return !document.hidden; }

/** Gaveta, contador e notificador compartilham cache, requisição em voo e ETag. */
export function atualizarAtividade(forcar = false): Promise<void> {
  if (emVoo) return emVoo;
  if (!forcar && Date.now() - ultimaLeitura < 1_500) return Promise.resolve();
  const versao = geracao;
  emVoo = requestSeMudou<ItemDeAtividade[]>("/atividade").then(itens => {
    if (versao !== geracao) return; // POST de vistos mais recente que este GET.
    // O transporte local antigo não repassa ETag: compara também o corpo para não renderizar o
    // App inteiro a cada poll. Em HTTP, 304 já conserva a referência sem serializar de novo.
    const mudou = itens !== null && JSON.stringify(itens) !== JSON.stringify(estado.itens);
    if (mudou || !estado.carregado || estado.erro) publicar({ itens: mudou ? itens! : estado.itens, carregado: true, erro: null });
    ultimaLeitura = Date.now();
  }).catch(motivo => { falhou(motivo); throw motivo; }).finally(() => { emVoo = null; });
  return emVoo;
}

export async function marcarAtividadeVista(ids: string[]) {
  if (!ids.length) return;
  // Divide lotes grandes sem extrapolar o limite de corpo da API.
  for (let i = 0; i < ids.length; i += 50) {
    const lote = ids.slice(i, i + 50);
    await request("/atividade/vistos", { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify({ ids: lote }) });
    geracao += 1;
    publicar({ ...estado, itens: estado.itens.map(item => lote.includes(item.id) ? { ...item, visto: true } : item), erro: null });
  }
}

export function useAtividade(emSegundoPlano = false) {
  const dados = useSyncExternalStore(assinar, () => estado);
  usePoll(atualizarAtividade, falhou, [], 2_000, emSegundoPlano);
  return { ...dados, atualizar: () => atualizarAtividade(true), marcar: marcarAtividadeVista };
}

/** Só uma conversa que está na frente de alguém conta como vista. O id do evento limita o POST:
 * um pedido que chegou depois da abertura não é marcado por uma resposta antiga. */
export function useAtividadeVista(conversas: string[], visivel: boolean) {
  const { itens } = useAtividade();
  const paginaVisivel = useSyncExternalStore(assinarVisibilidade, estaVisivel);
  const assinatura = JSON.stringify(itens.filter(item => !item.visto && conversas.includes(item.conversa)).map(item => item.id));
  useEffect(() => {
    if (!visivel || !paginaVisivel) return;
    const ids = JSON.parse(assinatura) as string[];
    if (ids.length) void marcarAtividadeVista(ids).catch(falhou);
  }, [assinatura, visivel, paginaVisivel]);
}
