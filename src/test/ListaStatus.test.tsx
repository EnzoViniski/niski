import { render, screen } from "@testing-library/react";
import { ConversationList } from "../mobile/MobileApp";
import type { Conversation } from "../mobile/api";

const conversa = (id: string, over: Partial<Conversation>): Conversation =>
  ({ id, title: id, project_id: "p", provider: "claude", profile_id: null, state: "answered", capabilities: null, ...over });

it("lista da web/celular mostra o mesmo status da lateral do PC", () => {
  render(<ConversationList perfis={[]} abrir={() => undefined} vazio="" naoVistas={new Set(["vista-nao"])} conversations={[
    conversa("rodando", { state: "working" }),
    conversa("aprovar", { capabilities: { prompt: false, approve: true, revision: "r", approval_text: null, reason: null } }),
    conversa("caiu", { notice: "api_error" }),
    conversa("vista-nao", {}),
    conversa("quieta", {}),
  ]} />);
  const status = (titulo: string) => screen.getByRole("button", { name: `Abrir conversa ${titulo}` }).dataset.status;
  expect(status("rodando")).toBe("trabalhando");
  expect(status("aprovar")).toBe("aprovacao");
  expect(status("caiu")).toBe("interrompido");
  expect(status("vista-nao")).toBe("finalizado");
  expect(status("quieta")).toBe("neutro");
  expect(screen.getByText("interrompido")).toBeInTheDocument();
});
