import { expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { BlocoDeAcoes, MessageBubble } from "../mobile/Chat";

it("bloco de código mostra a linguagem no topo", () => {
  render(<MessageBubble lado="agent" provider="claude" texto={"Veja:\n\n```ts\nconst a = 1;\n```"} />);
  expect(screen.getByText("ts")).toHaveClass("codigo-linguagem");
  expect(screen.getByText("const a = 1;")).toBeInTheDocument();
});

it("avatar só na primeira fala do turno", () => {
  const { container, rerender } = render(<MessageBubble lado="agent" provider="claude" texto="primeira" />);
  expect(container.querySelector(".bubble-avatar")).not.toBeNull();
  rerender(<MessageBubble lado="agent" provider="claude" texto="segunda" continua />);
  expect(container.querySelector(".bubble-avatar")).toBeNull();
});

it("comando aparece com $ e leitura de arquivo com ícone próprio", () => {
  const { container } = render(<BlocoDeAcoes aoVivo acoes={[
    { id: "1", role: "assistant", text: "npm test", provider: "claude", kind: "tool", tool: "Bash" },
    { id: "2", role: "assistant", text: "src/App.tsx", provider: "claude", kind: "tool", tool: "Read" },
  ]} />);
  expect(container.querySelectorAll(".acao-prompt").length).toBeGreaterThan(0);
  expect(screen.getByText("Leu")).toBeInTheDocument();
  expect(container.querySelectorAll("svg.acao-icone").length).toBeGreaterThan(0);
});
