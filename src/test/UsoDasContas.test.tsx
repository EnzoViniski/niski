import { expect, it, vi } from "vitest";
import { render, screen, within } from "@testing-library/react";
import { UsoDasContas } from "../web/UsoDasContas";

it("mostra 5H e semana de cada conta, com a cor da faixa e a conta da conversa destacada", async () => {
  vi.stubGlobal("fetch", vi.fn(async () => new Response(JSON.stringify([
    { id: "emailuber", provider: "claude", name: "EmailUber", usage: { status: "ok", reason: null,
      primary: { used_percent: 33, resets_at: null, reset_label: "Resets 8pm" }, secondary: { used_percent: 90, resets_at: null, reset_label: null } } },
    { id: "cdx", provider: "codex", name: "Codex", usage: null },
  ]))));
  render(<UsoDasContas contaAtiva="emailuber" />);

  const conta = (await screen.findByText("EmailUber")).closest("li")!;
  expect(conta).toHaveClass("w-uso-conta-ativa");
  expect(within(conta).getByText("33%")).toHaveClass("text-success");
  // Acima de 85% é vermelho, como no rodapé do app.
  expect(within(conta).getByText("90%")).toHaveClass("text-danger");
  expect(screen.getByText("Sem leitura de uso ainda")).toBeInTheDocument();
});
