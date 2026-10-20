import { useAtividade } from "../../features/atividade/atividadeService";
import { ListaDeAtividade } from "../../features/atividade/ListaDeAtividade";

/** Mesma caixa do PC, inclusive os vistos; sem push ou permissão de notificação no celular. */
export function Atividade({ abrirConversa }: { abrirConversa: (id: string) => void }) {
  const dados = useAtividade();
  return <ListaDeAtividade {...dados} abrir={item => abrirConversa(item.conversa)} />;
}
