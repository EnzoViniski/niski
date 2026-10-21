import { afterEach, describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MessageBubble } from "../mobile/Chat";
import { ChipDeTokens, dicaDosTokens, tokensCurtos } from "../mobile/conversa/TokensDaSessao";

/*
 * Fork (Conversa, 21/10/2026): horário e "Copiar" embaixo de cada mensagem (sua e do agente) e o total
 * de tokens da conversa no topo da aba.
 */

afterEach(() => vi.unstubAllGlobals());

function comClipboard() {
  const writeText = vi.fn(async () => undefined);
  Object.defineProperty(navigator, "clipboard", { value: { writeText }, configurable: true });
  return writeText;
}

describe("rodapé da mensagem", () => {
  it("sua mensagem também tem horário e Copiar, que copia tudo o que você mandou", async () => {
    const writeText = comClipboard();
    const { container } = render(<MessageBubble lado="user" provider="claude" texto="conserta o login do celular" horario="2026-10-21T20:51:00" />);
    const rodape = container.querySelector(".bubble-rodape.is-voce")!;
    expect(rodape.querySelector("time")?.textContent).toMatch(/\d{2}:\d{2}/);
    // Fora do balão: o balão colorido fica só com o que você escreveu.
    expect(container.querySelector(".bubble-user .bubble-rodape")).toBeNull();
    await userEvent.click(screen.getByRole("button", { name: "Copiar mensagem" }));
    expect(writeText).toHaveBeenCalledWith("conserta o login do celular");
  });

  it("resposta do agente: horário e Copiar embaixo", async () => {
    const writeText = comClipboard();
    const { container } = render(<MessageBubble lado="agent" provider="claude" texto="Achei a causa." horario="2026-10-21T20:55:00" />);
    expect(container.querySelector(".bubble-rodape.is-agente time")).not.toBeNull();
    await userEvent.click(screen.getByRole("button", { name: "Copiar resposta" }));
    expect(writeText).toHaveBeenCalledWith("Achei a causa.");
  });

  it("enviando: mostra o aviso no lugar do horário e ainda não oferece Copiar", () => {
    comClipboard();
    const { container } = render(<MessageBubble lado="user" provider="claude" texto="oi" pendente />);
    expect(container.querySelector(".bubble-rodape")?.textContent).toContain("enviando…");
    expect(screen.queryByRole("button", { name: "Copiar mensagem" })).toBeNull();
  });
});

describe("tokens da conversa", () => {
  it("formata curto em pt-BR", () => {
    expect(tokensCurtos(850)).toBe("850");
    expect(tokensCurtos(12_400)).toBe("12,4 mil");
    expect(tokensCurtos(1_284_530)).toBe("1,3 mi");
  });

  it("chip com o total e a conta detalhada no título; sem uso, nada", () => {
    const tokens = { total: 1_284_530, entrada: 1_251_200, cache: 1_180_400, saida: 33_330 };
    const { container, rerender } = render(<ChipDeTokens tokens={tokens} />);
    const chip = container.querySelector(".chip-tokens")!;
    expect(chip.textContent).toBe("1,3 mi tokens");
    expect(chip.getAttribute("title")).toBe(dicaDosTokens(tokens));
    expect(dicaDosTokens(tokens)).toContain("1.180.400 do cache");
    rerender(<ChipDeTokens tokens={null} />);
    expect(container.querySelector(".chip-tokens")).toBeNull();
  });
});
