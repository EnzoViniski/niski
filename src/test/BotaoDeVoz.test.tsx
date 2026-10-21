import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "./interacoesTelas";
import { invoke } from "@tauri-apps/api/core";
import { BotaoDeVoz } from "../mobile/BotaoDeVoz";

const pronto = { modelo_pronto: true, baixando: false, baixado_bytes: 0, total_bytes: 0, gravando: false };
beforeEach(() => {
  vi.mocked(invoke).mockReset().mockImplementation(async (cmd) => {
    if (cmd === "voz_estado") return pronto;
    if (cmd === "voz_transcrever") return "Revise o contraste da tela";
    if (cmd === "voz_previa") return "Revise o contraste";
    return null;
  });
});
async function gravar() {
  await userEvent.click(screen.getByRole("button", { name: "Falar (transcrição neste Mac)" }));
  return screen.findByRole("button", { name: /Parar e transcrever/ });
}
it("inicia a gravação e entrega a transcrição para revisão no campo", async () => {
  const onDitado = vi.fn();
  const onPrevia = vi.fn();
  render(<BotaoDeVoz onDitado={onDitado} onPrevia={onPrevia} travado={false} />);
  const parar = await gravar();
  expect(parar).toHaveAttribute("aria-pressed", "true");
  expect(onPrevia).toHaveBeenLastCalledWith({ texto: "", corrigindo: false });
  expect(onDitado).not.toHaveBeenCalled();
  await userEvent.click(parar);
  await waitFor(() => expect(onDitado).toHaveBeenCalledWith("Revise o contraste da tela"));
  expect(await screen.findByRole("button", { name: "Falar (transcrição neste Mac)" })).toBeEnabled();
  expect(onPrevia).toHaveBeenLastCalledWith(null);
});
it("confirma pelo ⏎ ao lado do microfone", async () => {
  const onDitado = vi.fn();
  render(<BotaoDeVoz onDitado={onDitado} travado={false} />);
  await gravar();
  await userEvent.click(screen.getByRole("button", { name: "Confirmar a transcrição" }));
  await waitFor(() => expect(onDitado).toHaveBeenCalledWith("Revise o contraste da tela"));
});
it.each(["botão", "Escape"])("descarta a gravação por %s", async (modo) => {
  const onDitado = vi.fn();
  render(<BotaoDeVoz onDitado={onDitado} travado={false} />);
  await gravar();
  if (modo === "botão") await userEvent.click(screen.getByRole("button", { name: "Descartar a gravação" }));
  else fireEvent.keyDown(window, { key: "Escape" });
  await waitFor(() => expect(invoke).toHaveBeenCalledWith("voz_cancelar"));
  expect(onDitado).not.toHaveBeenCalled();
  expect(screen.getByRole("button", { name: "Falar (transcrição neste Mac)" })).toBeInTheDocument();
});
it("Enter para e transcreve sem propagar a tecla ao campo de mensagem", async () => {
  const onDitado = vi.fn();
  const aoCampo = vi.fn();
  render(<div onKeyDown={aoCampo}><input aria-label="Mensagem" /><BotaoDeVoz onDitado={onDitado} travado={false} /></div>);
  await gravar();
  fireEvent.keyDown(screen.getByRole("textbox", { name: "Mensagem" }), { key: "Enter" });
  await waitFor(() => expect(onDitado).toHaveBeenCalledOnce());
  expect(aoCampo).not.toHaveBeenCalled();
});
it("oferece baixar o modelo e permite adiar", async () => {
  vi.mocked(invoke).mockResolvedValue({ ...pronto, modelo_pronto: false });
  render(<BotaoDeVoz onDitado={vi.fn()} travado={false} />);
  await userEvent.click(screen.getByRole("button", { name: "Falar (transcrição neste Mac)" }));
  expect(await screen.findByRole("dialog", { name: "Baixar modelo de voz" })).toBeInTheDocument();
  await userEvent.click(screen.getByRole("button", { name: "Agora não" }));
  expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  expect(invoke).not.toHaveBeenCalledWith("voz_gravar");
  expect(invoke).not.toHaveBeenCalledWith("voz_baixar_modelo");
});
it("baixa o modelo antes de começar a gravação", async () => {
  vi.mocked(invoke).mockResolvedValue({ ...pronto, modelo_pronto: false });
  render(<BotaoDeVoz onDitado={vi.fn()} travado={false} />);
  await userEvent.click(screen.getByRole("button", { name: "Falar (transcrição neste Mac)" }));
  await userEvent.click(await screen.findByRole("button", { name: "Baixar", exact: true }));
  expect(await screen.findByRole("button", { name: /Parar e transcrever/ })).toBeInTheDocument();
  expect(invoke).toHaveBeenCalledWith("voz_baixar_modelo");
  expect(invoke).toHaveBeenCalledWith("voz_gravar");
});
it("mostra silêncio como aviso e permite fechá-lo", async () => {
  vi.mocked(invoke).mockImplementation(async (cmd) => cmd === "voz_estado" ? pronto : cmd === "voz_transcrever" ? "   " : null);
  const onDitado = vi.fn();
  render(<BotaoDeVoz onDitado={onDitado} travado={false} />);
  await userEvent.click(await gravar());
  expect(await screen.findByRole("alert")).toHaveTextContent("Não ouvi nada");
  expect(onDitado).not.toHaveBeenCalled();
  await userEvent.click(screen.getByRole("button", { name: "Fechar" }));
  expect(screen.queryByRole("alert")).not.toBeInTheDocument();
});
it("exibe falha do microfone sem emitir ditado", async () => {
  vi.mocked(invoke).mockImplementation(async (cmd) => { if (cmd === "voz_estado") return pronto; throw new Error("Microfone sem permissão"); });
  const onDitado = vi.fn();
  render(<BotaoDeVoz onDitado={onDitado} travado={false} />);
  await userEvent.click(screen.getByRole("button", { name: "Falar (transcrição neste Mac)" }));
  expect(await screen.findByRole("alert")).toHaveTextContent("Microfone sem permissão");
  expect(onDitado).not.toHaveBeenCalled();
});
it("bloqueia nova gravação quando travado, mas permite parar a atual", async () => {
  const props = { onDitado: vi.fn(), travado: true };
  const { rerender } = render(<BotaoDeVoz {...props} />);
  expect(screen.getByRole("button", { name: "Falar (transcrição neste Mac)" })).toBeDisabled();
  rerender(<BotaoDeVoz {...props} travado={false} />);
  await gravar();
  rerender(<BotaoDeVoz {...props} />);
  expect(screen.getByRole("button", { name: /Parar e transcrever/ })).toBeEnabled();
});
it("solta o microfone ao desmontar a conversa", async () => {
  const { unmount } = render(<BotaoDeVoz onDitado={vi.fn()} travado={false} />);
  await gravar();
  unmount();
  await waitFor(() => expect(invoke).toHaveBeenCalledWith("voz_cancelar"));
});
it("passa a prévia ao vivo e as barrinhas da voz antes da transcrição final", async () => {
  vi.useFakeTimers();
  vi.mocked(invoke).mockImplementation(async (cmd) => {
    if (cmd === "voz_estado") return pronto;
    if (cmd === "voz_previa") return "Revise o contraste";
    if (cmd === "voz_niveis") return [0, 0.05, 0.8, 1];
    return null;
  });
  try {
    const onPrevia = vi.fn();
    const { container } = render(<BotaoDeVoz onDitado={vi.fn()} onPrevia={onPrevia} travado={false} />);
    fireEvent.click(screen.getByRole("button", { name: "Falar (transcrição neste Mac)" }));
    await act(async () => { await vi.advanceTimersByTimeAsync(0); });
    await act(async () => { await vi.advanceTimersByTimeAsync(1000); });
    expect(onPrevia).toHaveBeenLastCalledWith({ texto: "Revise o contraste", corrigindo: false });
    expect(invoke).toHaveBeenCalledWith("voz_niveis", { barras: 18 });
    expect(container.querySelectorAll(".voz-onda-barra")).toHaveLength(2);
    expect(container.querySelectorAll(".voz-onda-ponto")).toHaveLength(2);
  } finally { vi.useRealTimers(); }
});
