import { ATUALIZACAO_AUTOMATICA } from "../lib/identidade";
// Fork (atualização automática): a mesma checagem da barra (ao abrir e a cada 30 min), sem consulta própria.
import { useAtualizacaoRemota } from "../lib/atualizacaoRemota";

// Fork (Sobre mim): a paleta (⌘K) também abre a seção "Sobre mim".
export type SettingsSection = "sobre" | "sobre-mim";

export interface Notificacao {
  id: string;
  titulo: string;
  detalhe: string;
  /** Seção das Configurações onde a ação de fato acontece. */
  secao: SettingsSection;
}

/**
 * Pendências que o usuário precisa ver: por enquanto, só versão nova do app.
 * O sino apenas avisa e leva até Configurações — instalar é o `UpdateCard`
 * (ou o "Atualizar agora" da barra).
 *
 * Falha de rede não vira notificação — o usuário não pode fazer nada a
 * respeito e um alerta permanente só irrita (a checagem automática é calada).
 */
export function useNotifications(): Notificacao[] {
  const { fase, versao } = useAtualizacaoRemota({ automatica: ATUALIZACAO_AUTOMATICA });
  if (!ATUALIZACAO_AUTOMATICA || fase !== "disponivel" || !versao) return [];
  return [{
    id: `update:${versao}`,
    titulo: `Versão ${versao} disponível`,
    detalhe: "Atualize sem reinstalar o aplicativo.",
    secao: "sobre",
  }];
}
