import { useEffect, useRef } from "react";
import type { Conversation } from "../../mobile/api";
import type { ItemDeAtividade } from "./tipos";

const EVENTO = "medza-abrir-atividade";
export function pedirAbrirConversaDaAtividade(item: ItemDeAtividade) {
  // Fork: uma agenda adiada sem conversa abre o painel da agenda.
  if (item.tipo === "agendamento" && item.conversa.startsWith("agendamento:")) { window.dispatchEvent(new Event("medza-abrir-agendamentos")); return; }
  window.dispatchEvent(new CustomEvent<ItemDeAtividade>(EVENTO, { detail: item }));
}
/** Usa o mesmo caminho da paleta: seleciona workspace/projeto e abre a conversa, inclusive sem PTY. */
export function useAberturasDaAtividade(abrir: (conversa: Conversation) => void) {
  const atual = useRef(abrir); atual.current = abrir;
  useEffect(() => {
    const ouvir = (evento: Event) => {
      const item = (evento as CustomEvent<ItemDeAtividade>).detail;
      atual.current({ id: item.conversa, title: item.titulo, project_id: item.projeto_id,
        provider: item.plataforma, profile_id: item.conta_id, state: null, capabilities: null });
    };
    window.addEventListener(EVENTO, ouvir);
    return () => window.removeEventListener(EVENTO, ouvir);
  }, []);
}
