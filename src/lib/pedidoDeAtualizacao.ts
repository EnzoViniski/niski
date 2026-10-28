// "Atualizar o PC" pedido pelo celular. O engine deixa o pedido na pasta de dados
// (`omni-engine/src/mobile/atualizar_pc.rs`); aqui a interface confere a cada poucos segundos e faz
// o mesmo que o botão da barra:
// - versão nova no servidor → "Atualizar agora" (cópia do estado, download assinado, reabrir);
// - `.app` já trocado no disco (atualizado por fora) → reabrir trocando também o engine.
import { useEffect } from "react";
import { atualizarAgora, estadoDaAtualizacaoRemota, procurarAtualizacao, reiniciarParaAtualizar } from "./atualizacaoRemota";

export const INTERVALO_DO_PEDIDO_MS = 15_000;

export type ResultadoDoPedido = "sem-pedido" | "atualizando" | "reabrindo" | "em-dia";

export interface Dependencias {
  consumirPedido: () => Promise<boolean>;
  procurar: () => Promise<void>;
  haVersaoNova: () => boolean;
  atualizar: () => Promise<void>;
  versaoNovaNoDisco: () => Promise<boolean>;
  reabrir: () => Promise<void>;
}

async function chamar<T>(comando: string): Promise<T> {
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<T>(comando);
}

const PADRAO: Dependencias = {
  consumirPedido: () => chamar<boolean>("consumir_pedido_de_atualizacao"),
  procurar: () => procurarAtualizacao({ silencioso: true }),
  haVersaoNova: () => estadoDaAtualizacaoRemota().fase === "disponivel",
  atualizar: atualizarAgora,
  versaoNovaNoDisco: async () => (await chamar<{ disponivel: boolean }>("versao_nova_no_disco")).disponivel,
  reabrir: reiniciarParaAtualizar,
};

export async function atenderPedidoDeAtualizacao(dep: Dependencias = PADRAO): Promise<ResultadoDoPedido> {
  if (!(await dep.consumirPedido().catch(() => false))) return "sem-pedido";
  await dep.procurar().catch(() => undefined);
  if (dep.haVersaoNova()) {
    await dep.atualizar();
    return "atualizando";
  }
  if (await dep.versaoNovaNoDisco().catch(() => false)) {
    await dep.reabrir();
    return "reabrindo";
  }
  return "em-dia";
}

/** Liga a conferência do pedido enquanto o app estiver aberto (fora do Tauri não faz nada). */
export function useAtenderPedidoDeAtualizacao() {
  useEffect(() => {
    if (!("__TAURI_INTERNALS__" in window)) return;
    let ocupado = false;
    const conferir = () => {
      if (ocupado) return;
      ocupado = true;
      void atenderPedidoDeAtualizacao().finally(() => { ocupado = false; });
    };
    // Primeira conferência depois de o app assentar: a casca precisa ter decidido se troca o engine
    // agora ou adia (agente trabalhando) para o "versao_nova_no_disco" responder certo.
    const primeira = window.setTimeout(conferir, 10_000);
    const relogio = window.setInterval(conferir, INTERVALO_DO_PEDIDO_MS);
    return () => { window.clearTimeout(primeira); window.clearInterval(relogio); };
  }, []);
}
