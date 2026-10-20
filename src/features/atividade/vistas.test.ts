import { expect, it } from "vitest";
import { conversasVisiveis } from "./vistas";
import type { PaneNode, Project } from "../../types/workspace";
import type { TerminalSession } from "../terminal/terminalService";

it("só marca abas de conversa visíveis, respeitando painel maximizado e abas em segundo plano", () => {
  const primeiro: PaneNode = { type: "pane", id: "a", activeTabId: "tab-a", tabs: [
    { id: "tab-a", kind: "agent", title: "Agente", resourceId: "s" },
    { id: "oculta", kind: "conversa-historica", title: "Oculta", resourceId: "c-oculta" },
  ] };
  const segundo: PaneNode = { type: "pane", id: "b", activeTabId: "tab-b", tabs: [
    { id: "tab-b", kind: "conversa-historica", title: "Histórico", resourceId: "c-historico" },
  ] };
  const projeto = { id: "p", layout: { type: "split", id: "split", direction: "horizontal", ratio: 0.5, first: primeiro, second: segundo }, maximizedPaneId: null } as Project;
  const sessoes = [{ id: "s", conversation_id: "c-viva" }] as TerminalSession[];
  expect(conversasVisiveis(projeto, sessoes)).toEqual(["c-viva", "c-historico"]);
  expect(conversasVisiveis({ ...projeto, maximizedPaneId: "a" }, sessoes)).toEqual(["c-viva"]);
  expect(conversasVisiveis({ ...projeto, layout: { ...primeiro, tabs: [{ id: "tab-a", kind: "orquestracao", title: "Fluxo", resourceId: "c-viva" }] } }, sessoes)).toEqual([]);
  expect(conversasVisiveis(null, sessoes)).toEqual([]);
});
