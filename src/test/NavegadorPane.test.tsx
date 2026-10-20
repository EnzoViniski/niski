import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "./interacoesTelas";
import { invoke } from "@tauri-apps/api/core";
import { NavegadorPane } from "../features/navegador/NavegadorPane";

const tab = { id: "web-1", kind: "web" as const, title: "Documentação", resourceId: "https://example.test/inicio" };
beforeEach(() => { vi.mocked(invoke).mockReset().mockResolvedValue(undefined); });
it("mostra endereço e explica a limitação fora do app", () => {
  render(<NavegadorPane tab={tab} />);
  expect(screen.getByRole("textbox", { name: "Endereço" })).toHaveValue(tab.resourceId);
  expect(screen.getByText(/só funciona no app do computador/)).toBeInTheDocument();
});
it.each(["example.test/ajuda", "https://example.test/ajuda"])("navega ao enviar %s", async (texto) => {
  render(<NavegadorPane tab={tab} />);
  const campo = screen.getByRole("textbox", { name: "Endereço" });
  await userEvent.clear(campo);
  await userEvent.type(campo, `${texto}{Enter}`);
  await waitFor(() => expect(invoke).toHaveBeenCalledWith("navegador_acao", { rotulo: "nav-web-1", acao: "ir", url: "https://example.test/ajuda" }));
  expect(campo).toHaveValue("https://example.test/ajuda");
});
it.each([["Voltar", "voltar"], ["Avançar", "avancar"], ["Recarregar", "recarregar"]])("aciona %s", async (nome, acao) => {
  render(<NavegadorPane tab={tab} />);
  await userEvent.click(screen.getByRole("button", { name: nome }));
  await waitFor(() => expect(invoke).toHaveBeenCalledWith("navegador_acao", expect.objectContaining({ rotulo: "nav-web-1", acao })));
});
it("descarta edição incompleta ao sair do campo e abre o endereço atual fora", async () => {
  render(<NavegadorPane tab={tab} />);
  const campo = screen.getByRole("textbox", { name: "Endereço" });
  await userEvent.clear(campo);
  await userEvent.type(campo, "rascunho");
  await userEvent.click(screen.getByRole("button", { name: "Abrir no navegador do Mac" }));
  expect(campo).toHaveValue(tab.resourceId);
  await waitFor(() => expect(invoke).toHaveBeenCalledWith("navegador_abrir_fora", { url: tab.resourceId }));
});
it("mostra erro nativo e limpa o aviso na próxima ação", async () => {
  vi.mocked(invoke).mockRejectedValueOnce("Página indisponível");
  render(<NavegadorPane tab={tab} />);
  await userEvent.click(screen.getByRole("button", { name: "Recarregar" }));
  expect(await screen.findByRole("alert")).toHaveTextContent("Página indisponível");
  await userEvent.click(screen.getByRole("button", { name: "Voltar" }));
  await waitFor(() => expect(screen.queryByRole("alert")).not.toBeInTheDocument());
});
