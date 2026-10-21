import { useCallback, useEffect, useRef, useState } from "react";
import { AgentIcon } from "../components/ui/AgentIcon";
import { LoaderIcon, PlayIcon } from "../components/ui/PixelIcon";
import { Select } from "../components/ui/Select";
import { ApiError, request } from "./api";
import { lembrarAjustes } from "../lib/escolhasSalvas";
import { NOME_CURTO } from "../lib/identidade";
// Fork: modelo e esforço num botão só, com a barra de esforço (`conversa/SeletorDeModelo.tsx`).
import { SeletorDeModelo } from "./conversa/SeletorDeModelo";

/** `GET /conversas/{id}/opcoes`: o que dá para escolher nesta conversa e o que está valendo agora. */
export interface Opcoes {
  provider: string;
  profile_id: string | null;
  /** Modelo e esforço que a sessão usou por último (lidos do transcript). */
  model: string | null;
  effort: string | null;
  profiles: { id: string; name: string }[];
  models: { id: string; label: string; efforts: string[] }[];
  /** Sem id de sessão ainda (nenhuma mensagem), não há o que retomar com outros ajustes. */
  can_resume: boolean;
  /** Fork (chat-estavel): dá para trocar na sessão aberta — o Codex sem mensagem reabre com as
   *  opções novas. Engine antigo não manda: vale o `can_resume`. */
  can_adjust?: boolean;
}
export interface Ajustes { profile_id: string | null; model: string | null; effort: string | null }

/**
 * O modelo do transcript na lista: igual (`claude-opus-4-8`) ou o mais específico que ele contém
 * (`claude-opus-4-5-20251101` → `claude-opus-4-5`, não o apelido `opus`). Fork: a lista do Claude
 * passou a ter versões fixas além dos apelidos.
 */
export function modeloDaLista(opcoes: Pick<Opcoes, "model" | "models">): string {
  const atual = opcoes.model?.toLowerCase();
  if (!atual) return "";
  return opcoes.models.find((m) => m.id.toLowerCase() === atual)?.id
    ?? opcoes.models.filter((m) => atual.includes(m.id.toLowerCase()))
      .sort((a, b) => b.id.length - a.id.length)[0]?.id
    ?? "";
}

/**
 * Conta, modelo e esforço da conversa aberta, na faixa acima do campo de texto. Trocar qualquer um
 * reabre a conversa (`--resume`) com o ajuste novo: leva alguns segundos e nada se perde. Só com o
 * agente parado — no meio de um turno isso mataria o trabalho.
 */
export function AjustesDaConversa({ conversaId, pronto, onAplicar, onRetomar }: {
  conversaId: string;
  pronto: boolean;
  onAplicar: (ajustes: Ajustes) => Promise<void>;
  /** Conversa sem sessão aberta: em vez de "Aplicar", um "Retomar conversa" que abre a sessão já
   *  com a conta, o modelo e o esforço escolhidos. */
  onRetomar?: (ajustes: Ajustes) => Promise<void>;
}) {
  const [opcoes, setOpcoes] = useState<Opcoes | null>(null);
  const [conta, setConta] = useState("");
  const [modelo, setModelo] = useState("");
  const [esforco, setEsforco] = useState("");
  const [aplicando, setAplicando] = useState(false);
  // Engine que ainda não tem a rota (o front atualiza sozinho em dev; o engine só ao reiniciar).
  const [engineAntigo, setEngineAntigo] = useState(false);
  // Fork (chat-estavel): o que foi aplicado e o que se lia antes. O transcript só mostra o modelo novo
  // depois da próxima resposta; até lá, reler a mesma leitura antiga não desfaz a escolha na tela.
  const aplicado = useRef<{ antes: Pick<Opcoes, "model" | "effort">; depois: Ajustes } | null>(null);
  const lidoDoEngine = useRef<Pick<Opcoes, "model" | "effort"> | null>(null);
  // Fork: valores mais novos para o aplicar automático (ver `aplicarSeMudou`).
  const atual = useRef({ conta, modelo, esforco });
  atual.current = { conta, modelo, esforco };

  const carregar = useCallback(async () => {
    try {
      let lidas = await request<Opcoes>(`/conversas/${encodeURIComponent(conversaId)}/opcoes`);
      // Resposta fora do formato (engine antigo, proxy) some com a faixa — nunca derruba a conversa.
      if (!Array.isArray(lidas?.models) || !Array.isArray(lidas?.profiles)) { setOpcoes(null); return; }
      lidoDoEngine.current = { model: lidas.model, effort: lidas.effort };
      const pendente = aplicado.current;
      if (pendente && lidas.model === pendente.antes.model && lidas.effort === pendente.antes.effort) {
        lidas = { ...lidas, model: pendente.depois.model, effort: pendente.depois.effort };
      } else aplicado.current = null;
      setOpcoes(lidas);
      setEngineAntigo(false);
      setConta(lidas.profile_id ?? "");
      setModelo(modeloDaLista(lidas));
      setEsforco(lidas.effort ?? "");
    } catch (motivo) {
      setOpcoes(null);
      setEngineAntigo(motivo instanceof ApiError && motivo.status === 404);
    }
  }, [conversaId]);
  // Fork (chat-estavel): relê quando o agente fica pronto — depois da primeira resposta ou de reabrir
  // com outros ajustes, o que vale muda (antes ficava a leitura da abertura até trocar de conversa).
  useEffect(() => { aplicado.current = null; lidoDoEngine.current = null; }, [conversaId]);
  useEffect(() => { void carregar(); }, [carregar, pronto]);

  if (engineAntigo) {
    return <span className="m-chip-aviso" title="O engine em execução é anterior a esta tela">{`Reinicie o ${NOME_CURTO} para trocar conta e modelo`}</span>;
  }
  if (!opcoes || opcoes.models.length === 0 && opcoes.profiles.length < 2) return null;

  const esforcos = opcoes.models.find((m) => m.id === modelo)?.efforts
    ?? opcoes.models[0]?.efforts ?? [];
  const podeTrocar = onRetomar ? opcoes.can_resume : opcoes.can_adjust ?? opcoes.can_resume;
  const bloqueado = onRetomar ? !podeTrocar || aplicando : !pronto || !podeTrocar || aplicando;
  const motivo = !podeTrocar ? "Mande a primeira mensagem antes de trocar."
    : !pronto ? "Espere o agente terminar para trocar." : undefined;

  // Fork (composer no estilo do Codex): aplica sozinho ao fechar o painel do seletor ou ao trocar a
  // conta — sem o botão "Aplicar", que aparecia ao lado e empurrava o seletor. Lê pelo ref para pegar
  // o valor mais novo (o fechar do painel vem de um ouvinte criado antes da última escolha).
  function aplicarSeMudou(troca: Partial<{ conta: string; modelo: string; esforco: string }> = {}) {
    if (onRetomar || !opcoes) return;
    const v = { ...atual.current, ...troca };
    const mudouAgora = v.conta !== (opcoes.profile_id ?? "") || v.modelo !== modeloDaLista(opcoes) || v.esforco !== (opcoes.effort ?? "");
    if (mudouAgora && !bloqueado) void aplicar(v);
  }

  async function aplicar(v: { conta: string; modelo: string; esforco: string } = { conta, modelo, esforco }) {
    const { conta, modelo, esforco } = v;
    setAplicando(true);
    try {
      // O erro já aparece no chat (quem chama mostra); aqui só não marca como aplicado.
      try { await (onRetomar ?? onAplicar)({ profile_id: conta || null, model: modelo || null, effort: esforco || null }); } catch { return; }
      // Fork: as próximas conversas deste agente já nascem com este modelo/esforço.
      lembrarAjustes(opcoes!.provider, { model: modelo || null, effort: esforco || null });
      // A sessão reabre em segundo plano; o modelo novo só aparece no transcript depois da próxima
      // resposta, então a escolha fica como está até lá.
      aplicado.current = { antes: lidoDoEngine.current ?? { model: opcoes!.model, effort: opcoes!.effort }, depois: { profile_id: conta || null, model: modelo || null, effort: esforco || null } };
      setOpcoes((atual) => atual && { ...atual, profile_id: conta || null, model: modelo || null, effort: esforco || null });
    } finally {
      setAplicando(false);
    }
  }

  const iconeConta = <AgentIcon provider={opcoes.provider} size={12} className="shrink-0" />;

  return (
    <>
      {opcoes.profiles.length > 1 && (
        <span className="m-chip" title={motivo ?? "Conta usada nesta conversa"}>
          <Select size="sm" value={conta} disabled={bloqueado} onChange={(nova) => { setConta(nova); aplicarSeMudou({ conta: nova }); }} placeholder="Conta"
            options={opcoes.profiles.map((p) => ({ value: p.id, label: p.name, icon: iconeConta }))} />
        </span>
      )}
      {opcoes.models.length > 0 && (
        <SeletorDeModelo modelos={opcoes.models} modelo={modelo} esforco={esforco} esforcos={esforcos}
          desativado={bloqueado} motivo={motivo} aplicando={aplicando} onModelo={setModelo} onEsforco={setEsforco}
          onFechar={() => aplicarSeMudou()} />
      )}
      {onRetomar ? (
        <button type="button" className="m-chip-aplicar m-chip-retomar" disabled={bloqueado} onClick={() => void aplicar()}
          title={motivo ?? "Abre esta conversa de novo no PC, de onde parou, com a conta, o modelo e o esforço escolhidos"}>
          {aplicando ? <LoaderIcon aria-hidden className="anexo-girando h-3.5 w-3.5" /> : <PlayIcon aria-hidden className="h-3.5 w-3.5" />}
          {aplicando ? "Retomando…" : "Retomar conversa"}
        </button>
      ) : null}
    </>
  );
}
