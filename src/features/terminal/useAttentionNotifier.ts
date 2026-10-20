// Fork: o engine fornece os eventos e os vistos; a política de avisos mora na frente Atividade.
import { useAvisosDeAtividade } from "../atividade/avisos";
import type { PreferenciasDeAvisos } from "../atividade/tipos";
export { testarAviso } from "../atividade/avisos";

export function useAttentionNotifier(enabled: boolean, preferencias?: Partial<PreferenciasDeAvisos>) {
  useAvisosDeAtividade(enabled, preferencias);
}
