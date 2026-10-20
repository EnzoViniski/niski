// Fork (Onda 2.0): saiu do `MobileApp.tsx` sem mudança de lógica.
import type { Conversation, Projects } from "../api";
import { nomeDaPlataforma } from "../../lib/plataformasDeAgente";

/** Busca compartilhada pelo celular e pela web: todas as palavras precisam aparecer em algum dos
 * campos visíveis da conversa, da conta ou do projeto. */
export function filterConversations(
  conversations: Conversation[],
  query: string,
  perfis: Projects["profiles"] = [],
  projetos: Projects["projects"] = [],
): Conversation[] {
  const termos = query.toLocaleLowerCase("pt-BR").split(/\s+/).filter(Boolean);
  if (termos.length === 0) return conversations;
  const contas = new Map(perfis.map((perfil) => [perfil.id, perfil.name]));
  const pastas = new Map(projetos.map((projeto) => [projeto.id, `${projeto.name} ${projeto.path}`]));
  return conversations.filter((conversa) => {
    const texto = [
      conversa.title,
      conversa.provider,
      nomeDaPlataforma(conversa.provider),
      conversa.profile_id ? contas.get(conversa.profile_id) : null,
      pastas.get(conversa.project_id),
    ].filter(Boolean).join(" ").toLocaleLowerCase("pt-BR");
    return termos.every((termo) => texto.includes(termo));
  });
}
