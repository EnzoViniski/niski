import { useEffect, useRef } from "react";
import { NOME_DO_PRODUTO } from "../../lib/identidade";
import { useAtividade } from "./atividadeService";
import { preferenciasDeAvisos, type ItemDeAtividade, type PreferenciasDeAvisos, type SomDoAviso } from "./tipos";

export const EVENTO_AVISO_INTERNO = "medza-aviso-atividade";

/** Sons curtos distintos dentro do app; o sistema usa o nome nativo fora de foco. */
export async function tocarSom(som: SomDoAviso | null) {
  if (!som || typeof AudioContext === "undefined") return;
  const audio = new AudioContext();
  try {
    await audio.resume();
    const oscilador = audio.createOscillator();
    const ganho = audio.createGain();
    const agora = audio.currentTime;
    oscilador.type = som === "Basso" ? "sine" : "triangle";
    oscilador.frequency.setValueAtTime({ Glass: 1_320, Ping: 880, Basso: 220 }[som], agora);
    ganho.gain.setValueAtTime(0.0001, agora);
    ganho.gain.exponentialRampToValueAtTime(0.12, agora + 0.01);
    ganho.gain.exponentialRampToValueAtTime(0.0001, agora + 0.22);
    oscilador.connect(ganho); ganho.connect(audio.destination);
    oscilador.start(agora); oscilador.stop(agora + 0.24);
    oscilador.onended = () => { void audio.close(); };
  } catch (erro) { await audio.close(); throw erro; }
}

async function permissao() {
  const plugin = await import("@tauri-apps/plugin-notification");
  const permitido = await plugin.isPermissionGranted() || await plugin.requestPermission() === "granted";
  return { plugin, permitido };
}

async function notificarSistema(enviar: (aviso: { title: string; body: string; sound?: string }) => void,
  title: string, body: string, som: SomDoAviso | null) {
  // O backend Windows desta versão traduz sound por uma enumeração nativa: os nomes do macOS
  // não são sons válidos ali. Acompanha o toast com o mesmo som curto WebAudio usado em foco.
  const noWindows = /Windows/.test(navigator.userAgent);
  enviar({ title, body, ...(som && !noWindows ? { sound: som } : {}) });
  if (noWindows) await tocarSom(som);
}

/** Um lote é resumido depois de aplicar as preferências: tipos desligados nunca entram na conta. */
export async function enviarAvisos(itens: ItemDeAtividade[], configuracao?: Partial<PreferenciasDeAvisos>) {
  const preferencias = preferenciasDeAvisos(configuracao);
  const { getCurrentWindow, UserAttentionType } = await import("@tauri-apps/api/window");
  const janela = getCurrentWindow();
  const emFoco = await janela.isFocused();
  const ativos = itens.filter(item => preferencias[item.tipo].quando === "sempre"
    || (!emFoco && preferencias[item.tipo].quando === "fora_de_foco"));
  if (!ativos.length) return [];
  const lotes = ativos.length > 3 ? [ativos] : ativos.map(item => [item]);
  if (emFoco) {
    for (const lote of lotes) {
      window.dispatchEvent(new CustomEvent<ItemDeAtividade[]>(EVENTO_AVISO_INTERNO, { detail: lote }));
      await tocarSom(preferencias[lote[0].tipo].som);
    }
    return ativos;
  }
  const { plugin, permitido } = await permissao();
  if (permitido) for (const lote of lotes) {
    const item = lote[0];
    const som = preferencias[item.tipo].som;
    await notificarSistema(plugin.sendNotification,
      lote.length > 1 ? NOME_DO_PRODUTO : `${item.projeto} · ${item.conta}`,
      lote.length > 1 ? `${lote.length} agentes com atividade nova` : `${item.titulo} — ${item.resumo}`, som);
  }
  await janela.requestUserAttention(UserAttentionType.Informational);
  return ativos;
}

/** Um aviso por conversa até o engine registrar a leitura, inclusive se ela veio do celular. */
export function useAvisosDeAtividade(enabled: boolean, configuracao?: Partial<PreferenciasDeAvisos>) {
  const { itens, carregado } = useAtividade(true);
  const notificados = useRef<Set<string> | null>(null);
  const pendentes = useRef(new Map<string, string>());
  useEffect(() => {
    if (!carregado) return;
    for (const [conversa, id] of pendentes.current) {
      if (itens.some(item => item.id === id && item.visto)) pendentes.current.delete(conversa);
    }
    if (notificados.current === null) {
      notificados.current = new Set(itens.map(item => item.id));
      return; // Abrir o app não envia uma pilha de avisos antigos.
    }
    const novos = itens.filter(item => !notificados.current!.has(item.id));
    notificados.current = new Set(itens.map(item => item.id));
    const preferencias = preferenciasDeAvisos(configuracao);
    const avisar: ItemDeAtividade[] = [];
    for (const item of novos) {
      if (!enabled || item.visto || preferencias[item.tipo].quando === "nunca" || pendentes.current.has(item.conversa)) continue;
      pendentes.current.set(item.conversa, item.id);
      avisar.push(item);
    }
    if (avisar.length) void enviarAvisos(avisar, preferencias).then(enviados => {
      for (const item of avisar) if (!enviados.includes(item) && pendentes.current.get(item.conversa) === item.id) pendentes.current.delete(item.conversa);
    }).catch(motivo => {
      // Só metadados: nada de título, mensagem ou corpo da conversa nos logs.
      console.error("[atividade] falha ao enviar aviso", motivo instanceof Error ? motivo.name : "erro");
    });
  }, [itens, carregado, enabled, configuracao]);
}

/** O teste ignora o foco para permitir conferir a permissão do sistema. */
export async function testarAviso(som: SomDoAviso | null = "Glass"): Promise<string | null> {
  try {
    const { plugin, permitido } = await permissao();
    if (!permitido) return `O sistema negou permissão de notificação para o ${NOME_DO_PRODUTO}.`;
    await notificarSistema(plugin.sendNotification, NOME_DO_PRODUTO, "Teste de aviso — é assim que um agente te chama.", som);
    return null;
  } catch (motivo) { return motivo instanceof Error ? motivo.message : String(motivo); }
}
