import { useState } from "react";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "./interacoesTelas";
import { invoke } from "@tauri-apps/api/core";

// Fork: a voz só aparece no app do desktop; o Composer lê isso ao carregar o módulo.
vi.hoisted(() => { (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = {}; });
const { Composer } = await import("../mobile/Composer");

const pronto = { modelo_pronto: true, baixando: false, baixado_bytes: 0, total_bytes: 0, gravando: false };
let terminarTranscricao: (texto: string) => void = () => undefined;
beforeEach(() => {
  vi.mocked(invoke).mockReset().mockImplementation(async (cmd) => {
    if (cmd === "voz_estado") return pronto;
    if (cmd === "voz_previa") return "deixa o botão azul";
    if (cmd === "voz_transcrever") return new Promise<string>((pronta) => { terminarTranscricao = pronta; });
    return null;
  });
});

function ComposerDeTeste() {
  const [texto, setTexto] = useState("Oi,");
  return <Composer texto={texto} onTexto={setTexto} anexos={[]} onAdicionar={vi.fn()} onRemover={vi.fn()}
    onEnviar={vi.fn()} podeEnviar ocupado={false} placeholder="Responder" />;
}

it("mostra a prévia em cinza dentro do campo e troca pelo texto final ao confirmar", async () => {
  const { container } = render(<ComposerDeTeste />);
  const linha = container.querySelector(".composer-linha");
  // O microfone fica na linha do texto, não na barra de baixo.
  expect(linha?.contains(screen.getByRole("button", { name: "Falar (transcrição neste Mac)" }))).toBe(true);
  expect(container.querySelector(".composer-rodape .voz")).toBeNull();

  await userEvent.click(screen.getByRole("button", { name: "Falar (transcrição neste Mac)" }));
  const previa = await screen.findByText("deixa o botão azul", {}, { timeout: 2000 });
  expect(previa).toHaveClass("composer-previa");
  expect(screen.getByLabelText("Prévia do ditado")).toHaveTextContent("Oi, deixa o botão azul");
  expect(container.querySelector("textarea")).not.toBeVisible();

  await userEvent.click(screen.getByRole("button", { name: "Confirmar a transcrição" }));
  expect(await screen.findByText("deixa o botão azul")).toHaveClass("is-corrigindo");
  terminarTranscricao("deixa o botão azul.");
  await waitFor(() => expect(screen.queryByLabelText("Prévia do ditado")).not.toBeInTheDocument());
  const campo = container.querySelector("textarea");
  expect(campo).toBeVisible();
  expect(campo).toHaveValue("Oi, deixa o botão azul.");
});

it("descartar volta ao campo sem mexer no texto", async () => {
  const { container } = render(<ComposerDeTeste />);
  await userEvent.click(screen.getByRole("button", { name: "Falar (transcrição neste Mac)" }));
  await screen.findByLabelText("Prévia do ditado");
  await userEvent.click(screen.getByRole("button", { name: "Descartar a gravação" }));
  await waitFor(() => expect(screen.queryByLabelText("Prévia do ditado")).not.toBeInTheDocument());
  expect(container.querySelector("textarea")).toHaveValue("Oi,");
});
