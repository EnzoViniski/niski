import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import { MemoriaCompartilhada } from "../features/memoria/MemoriaCompartilhada";
import { engineApi } from "../features/terminal/terminalService";

vi.mock("../features/terminal/terminalService", () => ({ engineApi: vi.fn() }));

beforeEach(() => {
  vi.mocked(engineApi).mockReset().mockImplementation(async (method, _path, body) => ({
    type: "api", status: 200, body: method === "GET" ? JSON.stringify({ ativa: false, texto: "Preferências existentes" }) : body ?? "{}",
  }));
});

it("preserva o conteúdo existente e só salva depois da ação explícita", async () => {
  render(<MemoriaCompartilhada />);
  const campo = await screen.findByLabelText("Sobre mim e meu trabalho");
  expect(campo).toHaveValue("Preferências existentes");
  expect(screen.getByRole("button", { name: "Salvar memória" })).toBeDisabled();
  fireEvent.change(campo, { target: { value: "Enzo. Respostas curtas em português." } });
  fireEvent.click(screen.getByRole("checkbox"));
  expect(engineApi).toHaveBeenCalledTimes(1);
  fireEvent.click(screen.getByRole("button", { name: "Salvar memória" }));
  await screen.findByRole("status");
  expect(engineApi).toHaveBeenLastCalledWith("POST", "/memoria", JSON.stringify({ ativa: true, texto: "Enzo. Respostas curtas em português." }));
  expect(screen.getByRole("button", { name: "Salvar memória" })).toBeDisabled();
});

it("não oferece um formulário vazio por cima de uma memória que falhou ao carregar", async () => {
  vi.mocked(engineApi).mockResolvedValue({ type: "api", status: 500, body: JSON.stringify({ error: "Arquivo preservado" }) });
  render(<MemoriaCompartilhada />);
  expect(await screen.findByRole("alert")).toHaveTextContent("Arquivo preservado");
  expect(screen.queryByRole("textbox")).not.toBeInTheDocument();
});

it("mantém o texto editado quando falha ao salvar, permitindo tentar de novo", async () => {
  render(<MemoriaCompartilhada />);
  const campo = await screen.findByLabelText("Sobre mim e meu trabalho");
  fireEvent.change(campo, { target: { value: "Texto a preservar" } });
  vi.mocked(engineApi).mockResolvedValueOnce({ type: "api", status: 500, body: "{}" });
  fireEvent.click(screen.getByRole("button", { name: "Salvar memória" }));
  await screen.findByRole("alert");
  expect(campo).toHaveValue("Texto a preservar");
  await waitFor(() => expect(screen.getByRole("button", { name: "Salvar memória" })).not.toBeDisabled());
});
