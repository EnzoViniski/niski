import { useEffect, useState } from "react";
import { ReloadIcon } from "./ui/PixelIcon";

/*
 * Fork: "Reiniciar para atualizar". Quando um build novo cai por cima do app aberto
 * (`scripts/build-mac-local.mjs`), o Rust percebe pela data dos executáveis
 * (`src-tauri/src/atualizacao.rs`) e aparece este botão na barra: um clique e o app reabre
 * sozinho já com o código novo. Fora do Tauri não aparece.
 */

interface VersaoNova {
  disponivel: boolean;
  reinicia_agentes: boolean;
  /** Fork: agentes trabalhando agora; com algum, reabrir só troca a interface. */
  agentes_ocupados?: number;
  engine_pendente?: boolean;
}

const INTERVALO_MS = 15_000;

async function chamar<T>(comando: string): Promise<T> {
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<T>(comando);
}

export function AvisoDeAtualizacao() {
  const [versao, setVersao] = useState<VersaoNova | null>(null);
  const [confirmando, setConfirmando] = useState(false);

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

  // A confirmação some sozinha se ninguém clicar de novo.
  useEffect(() => {
    if (!confirmando) return;
    const id = window.setTimeout(() => setConfirmando(false), 5000);
    return () => window.clearTimeout(id);
  }, [confirmando]);

  if (!versao?.disponivel) return null;

  // Fork: com agente trabalhando, reabrir troca só a interface — o engine e os agentes seguem
  // rodando (`engine_client::ensure_engine` adia a troca). Parados todos, a troca do engine reabre as
  // conversas com o histórico, sem cortar trabalho nenhum.
  const ocupados = versao.agentes_ocupados ?? 0;
  const dica = ocupados > 0
    ? `Versão nova instalada. ${ocupados} ${ocupados === 1 ? "agente está" : "agentes estão"} trabalhando: reabrir atualiza a tela e eles seguem rodando. A parte que reinicia os agentes fica para quando todos pararem.`
    : versao.reinicia_agentes
    ? "Versão nova instalada. Nenhum agente trabalhando agora: reabrir reinicia as conversas abertas, que voltam com o histórico."
    : "Versão nova instalada. Os agentes seguem rodando.";

  function clicar() {
    if (versao?.reinicia_agentes && !confirmando) { setConfirmando(true); return; }
    void chamar("reiniciar_para_atualizar").catch(() => undefined);
  }

  const rotulo = confirmando ? "Confirmar: reabre as conversas"
    : ocupados > 0 ? "Atualizar (agentes seguem)"
    : versao.engine_pendente ? "Concluir atualização"
    : "Reiniciar para atualizar";

  return (
    <button type="button" className="titlebar-acao titlebar-atualizar" title={dica} onClick={clicar}>
      <ReloadIcon aria-hidden className="h-3.5 w-3.5" />
      {rotulo}
    </button>
  );
}
