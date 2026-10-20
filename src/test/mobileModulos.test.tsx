// Fork (Onda 2.0): o `MobileApp.tsx` virou módulos (`listas/`, `conversa/`). Quem importava dele
// continua importando — os nomes de lá são os mesmos objetos dos módulos novos.
import { fireEvent, render, screen } from "@testing-library/react";
import { vi } from "vitest";
import * as MobileApp from "../mobile/MobileApp";
import { filterConversations } from "../mobile/listas/filtro";
import { ConversationList, ConversasPorEstado } from "../mobile/listas/ConversationList";
import { ProjectList } from "../mobile/listas/ProjectList";
import { Ocultas } from "../mobile/listas/Ocultas";
import { NovaSessao } from "../mobile/listas/NovaSessao";
import { Secao } from "../mobile/listas/Secao";
import { ConversationDetail } from "../mobile/conversa/ConversationDetail";
import { ModoDoAgente } from "../mobile/conversa/ModoDoAgente";
import { agruparMensagens, juntarPaginas } from "../mobile/conversa/useTimeline";
import { CartaoDePermissao } from "../mobile/conversa/CartaoDePermissao";
import { CabecalhoDaConversa } from "../mobile/conversa/CabecalhoDaConversa";
import { haQuanto } from "../lib/tempo";
import type { Conversation } from "../mobile/api";

it("o MobileApp reexporta os mesmos nomes de antes", () => {
  expect(MobileApp.filterConversations).toBe(filterConversations);
  expect(MobileApp.ConversationList).toBe(ConversationList);
  expect(MobileApp.ConversasPorEstado).toBe(ConversasPorEstado);
  expect(MobileApp.ProjectList).toBe(ProjectList);
  expect(MobileApp.Ocultas).toBe(Ocultas);
  expect(MobileApp.NovaSessao).toBe(NovaSessao);
  expect(MobileApp.Secao).toBe(Secao);
  expect(MobileApp.ConversationDetail).toBe(ConversationDetail);
  expect(MobileApp.ModoDoAgente).toBe(ModoDoAgente);
  expect(MobileApp.agruparMensagens).toBe(agruparMensagens);
  expect(MobileApp.juntarPaginas).toBe(juntarPaginas);
  expect(MobileApp.haQuanto).toBe(haQuanto);
  expect(typeof MobileApp.MobileApp).toBe("function");
});

it("cartão de permissão responde permitir e negar, e trava ocupado", () => {
  const responder = vi.fn();
  const { rerender } = render(<CartaoDePermissao texto="Bash · npm test" busy={false} onResponder={responder} />);
  expect(screen.getByText("Aprovação pendente")).toBeTruthy();
  expect(screen.getByText("Bash · npm test")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Permitir" }));
  fireEvent.click(screen.getByRole("button", { name: "Negar" }));
  expect(responder.mock.calls).toEqual([[true], [false]]);
  rerender(<CartaoDePermissao texto="Bash · npm test" busy onResponder={responder} />);
  expect((screen.getByRole("button", { name: "Permitir" }) as HTMLButtonElement).disabled).toBe(true);
});

it("cabeçalho da conversa: agente, estado e projeto", () => {
  const conversa = { id: "c", title: "t", project_id: "p", provider: "antigravity", profile_id: null, state: "working",
    capabilities: null } as Conversation;
  const { container } = render(<CabecalhoDaConversa conversa={conversa} projeto={{ name: "loja" }} />);
  expect(container.textContent).toContain("Antigravity · Trabalhando · loja");
  expect(container.querySelector(".m-dot-ativo")).toBeTruthy();
});
