import { useEffect, useState } from "react";
import { ReloadIcon } from "./ui/PixelIcon";
// Fork (atualização automática): versão nova publicada → "Atualizar agora" (lógica em `lib/atualizacaoRemota`).
import {
  agentesTrabalhando,
  atualizarAgora,
  marcarReinicioDeAtualizacao,
  reiniciarParaAtualizar,
  textoDaConfirmacao,
  useAtualizacaoRemota,
  type EstadoRemoto,
} from "../lib/atualizacaoRemota";

/*
 * Fork: "Reiniciar para atualizar". Quando um build novo cai por cima do app aberto
 * (`scripts/build-mac-local.mjs`), o Rust percebe pela data dos executáveis
 * (`src-tauri/src/atualizacao.rs`) e aparece este botão na barra: um clique e o app reabre
 * sozinho já com o código novo. Fora do Tauri não aparece.
 *
 * Fork (retomar-sempre): com o engine também novo, o botão sempre pede confirmação e diz que o
 * engine será reiniciado (e quantos agentes param). Confirmado, o engine é encerrado de fato antes de
 * reabrir (`trocar_engine`) — antes, com algum agente trabalhando, o reinício só trocava a tela e a
 * interface nova seguia falando com o engine antigo. "Só a tela" continua possível.
 *
 * Fork (atualização automática): sem build no disco, a versão nova publicada (updater) aparece como
 * "Atualizar agora · v<x>": confirmação curta → progresso aqui mesmo → reabre com o engine novo.
 */

export interface VersaoNova {
  disponivel: boolean;
  reinicia_agentes: boolean;
  /** Fork: agentes trabalhando agora. */
  agentes_ocupados?: number;
  engine_pendente?: boolean;
  /** Fork (retomar-sempre): esta janela está falando com o engine antigo (troca adiada). */
  engine_antigo_rodando?: boolean;
}

const INTERVALO_MS = 15_000;

async function chamar<T>(comando: string, args?: Record<string, unknown>): Promise<T> {
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<T>(comando, args);
}

function agentes(n: number) {
  return `${n} ${n === 1 ? "agente trabalhando agora será interrompido" : "agentes trabalhando agora serão interrompidos"}`;
}

/** Textos do botão e da confirmação, separados para teste. */
export function textosDaAtualizacao(versao: VersaoNova) {
  const ocupados = versao.agentes_ocupados ?? 0;
  const engine = Boolean(versao.engine_pendente);
  const rotulo = !engine ? "Reiniciar para atualizar"
    : versao.engine_antigo_rodando ? "Engine desatualizado · atualizar"
    : "Atualizar app e engine";
  const dica = !engine
    ? "Versão nova instalada. Só a tela muda: os agentes seguem rodando."
    : versao.engine_antigo_rodando
    ? "A tela já é a nova, mas o engine (que roda os agentes) ainda é o antigo: algumas funções podem não responder até atualizar."
    : "Versão nova instalada, inclusive do engine que roda os agentes.";
  const confirmacao = `O engine também será reiniciado${ocupados > 0 ? `: ${agentes(ocupados)}` : ""}. As abas abertas voltam sozinhas com --resume (a mesma conversa, com o histórico); as que estiverem atrás de outra, quando você abrir.`;
  return { rotulo, dica, confirmacao, ocupados, engine };
}

export function AvisoDeAtualizacao() {
  const [versao, setVersao] = useState<VersaoNova | null>(null);
  const [confirmando, setConfirmando] = useState(false);
  const [reiniciando, setReiniciando] = useState(false);
  const remoto = useAtualizacaoRemota({ automatica: true });

  useEffect(() => {
    if (!("__TAURI_INTERNALS__" in window)) return;
    let vivo = true;
    const checar = () => void chamar<VersaoNova>("versao_nova_no_disco")
      .then((nova) => { if (vivo) setVersao(nova); })
      .catch(() => undefined);
    checar();
    const id = window.setInterval(checar, INTERVALO_MS);
    window.addEventListener("focus", checar);
    return () => { vivo = false; window.clearInterval(id); window.removeEventListener("focus", checar); };
  }, []);

  // A confirmação some sozinha se ninguém decidir.
  useEffect(() => {
    if (!confirmando) return;
    const id = window.setTimeout(() => setConfirmando(false), 20_000);
    const esc = (event: KeyboardEvent) => { if (event.key === "Escape") setConfirmando(false); };
    window.addEventListener("keydown", esc);
    return () => { window.clearTimeout(id); window.removeEventListener("keydown", esc); };
  }, [confirmando]);

  if (!versao?.disponivel) return <AtualizarAgora remoto={remoto} />;
  const { rotulo, dica, confirmacao, ocupados, engine } = textosDaAtualizacao(versao);

  function reiniciar(trocarEngine: boolean) {
    setReiniciando(true);
    // Fork: a próxima abertura sabe que foi atualização e religa as abas visíveis.
    if (trocarEngine) marcarReinicioDeAtualizacao();
    void chamar("reiniciar_para_atualizar", { trocarEngine }).catch(() => setReiniciando(false));
  }

  function clicar() {
    if (engine) { setConfirmando((atual) => !atual); return; }
    reiniciar(false);
  }

  return (
    <span className="relative flex self-stretch">
      <button type="button" className={`titlebar-acao titlebar-atualizar${confirmando ? " is-aberto" : ""}`} title={dica}
        aria-expanded={engine ? confirmando : undefined} onClick={clicar} disabled={reiniciando}>
        <ReloadIcon aria-hidden className="h-3.5 w-3.5" />
        {reiniciando ? "Reiniciando…" : rotulo}
      </button>
      {confirmando && (
        <div role="dialog" aria-label="Confirmar atualização"
          className="absolute right-0 top-full z-[70] mt-1 w-80 border-2 border-border-subtle bg-bg-elevated p-3 text-xs text-text-secondary shadow-lg">
          <p>{confirmacao}</p>
          <div className="mt-3 flex flex-wrap justify-end gap-2">
            <button type="button" className="px-2 py-1 text-text-muted hover:text-text-primary" onClick={() => setConfirmando(false)}>
              Cancelar
            </button>
            {ocupados > 0 && (
              <button type="button" className="border border-border-subtle px-2 py-1 hover:text-text-primary" disabled={reiniciando}
                title="Reabre só a tela; o engine antigo e os agentes seguem rodando até você atualizar de novo."
                onClick={() => reiniciar(false)}>
                Só a tela agora
              </button>
            )}
            <button type="button" className="bg-accent px-2 py-1 font-bold text-bg-primary hover:opacity-90" disabled={reiniciando}
              onClick={() => reiniciar(true)}>
              Reiniciar app e engine
            </button>
          </div>
        </div>
      )}
    </span>
  );
}

/** Fork (atualização automática): botão da versão publicada. Some quando não há nada a fazer. */
function AtualizarAgora({ remoto }: { remoto: EstadoRemoto }) {
  const [confirmacao, setConfirmacao] = useState<string | null>(null);

  useEffect(() => {
    if (confirmacao === null) return;
    const id = window.setTimeout(() => setConfirmacao(null), 20_000);
    const esc = (event: KeyboardEvent) => { if (event.key === "Escape") setConfirmacao(null); };
    window.addEventListener("keydown", esc);
    return () => { window.clearTimeout(id); window.removeEventListener("keydown", esc); };
  }, [confirmacao]);

  const { fase, versao, progresso, erro } = remoto;
  const tentouEFalhou = fase === "erro" && versao !== null;
  if (fase !== "disponivel" && fase !== "baixando" && fase !== "instalada" && fase !== "reiniciando" && !tentouEFalhou) return null;

  async function abrir() {
    if (confirmacao !== null) { setConfirmacao(null); return; }
    setConfirmacao(textoDaConfirmacao(versao, await agentesTrabalhando()));
  }

  const rotulo = fase === "baixando" ? `Baixando v${versao} · ${progresso}%`
    : fase === "reiniciando" ? "Reiniciando…"
    : fase === "instalada" ? "Reabrir agora"
    : tentouEFalhou ? `Tentar de novo · v${versao}`
    : `Atualizar agora · v${versao}`;
  const dica = erro ? `A atualização falhou: ${erro}` : fase === "instalada" ? "Versão nova instalada. Reabra para usar." : `Versão ${versao} disponível.`;
  const parado = fase === "baixando" || fase === "reiniciando";

  return (
    <span className="relative flex self-stretch">
      <button type="button" className={`titlebar-acao titlebar-atualizar relative${confirmacao !== null ? " is-aberto" : ""}`} title={dica}
        aria-expanded={fase === "instalada" ? undefined : confirmacao !== null} disabled={parado}
        onClick={() => { if (fase === "instalada") void reiniciarParaAtualizar(); else void abrir(); }}>
        <ReloadIcon aria-hidden className="h-3.5 w-3.5" />
        {rotulo}
        {fase === "baixando" && (
          <span className="absolute inset-x-0 bottom-0 h-0.5 bg-accent transition-all duration-300" style={{ width: `${progresso}%` }}
            role="progressbar" aria-label="Progresso da atualização" aria-valuenow={progresso} aria-valuemin={0} aria-valuemax={100} />
        )}
      </button>
      {confirmacao !== null && (
        <div role="dialog" aria-label="Confirmar atualização"
          className="absolute right-0 top-full z-[70] mt-1 w-80 border-2 border-border-subtle bg-bg-elevated p-3 text-xs text-text-secondary shadow-lg">
          <p>{confirmacao}</p>
          <div className="mt-3 flex flex-wrap justify-end gap-2">
            <button type="button" className="px-2 py-1 text-text-muted hover:text-text-primary" onClick={() => setConfirmacao(null)}>
              Cancelar
            </button>
            <button type="button" className="bg-accent px-2 py-1 font-bold text-bg-primary hover:opacity-90"
              onClick={() => { setConfirmacao(null); void atualizarAgora(); }}>
              Atualizar e reabrir
            </button>
          </div>
        </div>
      )}
    </span>
  );
}
