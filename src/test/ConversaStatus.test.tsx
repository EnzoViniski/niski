import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { EtiquetaDoStatus, statusDaConversa } from "../components/ConversaStatus";

describe("statusDaConversa", () => {
  it("traduz o estado do engine nos cinco status da lateral", () => {
    expect(statusDaConversa({ state: "working" }, false)).toBe("trabalhando");
    expect(statusDaConversa({ state: "approval_required" }, false)).toBe("aprovacao");
    expect(statusDaConversa({ state: "stopped", notice: "usage_limit" }, false)).toBe("interrompido");
    expect(statusDaConversa({ state: "answered", notice: "api_error" }, true)).toBe("interrompido");
    expect(statusDaConversa({ state: "crashed" }, false)).toBe("interrompido");
    expect(statusDaConversa({ state: "answered" }, true)).toBe("finalizado");
  });

  /** "answered" é tanto "acabou de responder" quanto "nada novo desde ontem": só o primeiro é
   *  finalizado — o resto é neutro, senão toda conversa parada pareceria pedir atenção. */
  it("terminou e já foi visto é neutro", () => {
    expect(statusDaConversa({ state: "answered" }, false)).toBe("neutro");
    expect(statusDaConversa({ state: "stopped" }, true)).toBe("neutro");
    expect(statusDaConversa({ state: "orphan" }, true)).toBe("neutro");
  });

  it("neutro não ganha etiqueta; os outros sim", () => {
    const { container, rerender } = render(<EtiquetaDoStatus status="neutro" />);
    expect(container).toBeEmptyDOMElement();
    rerender(<EtiquetaDoStatus status="trabalhando" />);
    expect(screen.getByText("trabalhando")).toBeInTheDocument();
  });
});
