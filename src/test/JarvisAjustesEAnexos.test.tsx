import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import userEvent from "./interacoesTelas";
import { JanelaDoJarvis } from "../features/jarvis/JanelaDoJarvis";
import { AjustesDoJarvis, type ConfiguracaoDoJarvis } from "../features/jarvis/AjustesDoJarvis";
import { MAXIMO_POR_ANEXO, prepararAnexos } from "../features/jarvis/anexosDoJarvis";
import { aplicarEvento, estadoInicial } from "../features/jarvis/estadoDoJarvis";

vi.mock("../features/jarvis/OrbeDoJarvis", () => ({ OrbeDoJarvis: () => <canvas aria-hidden /> }));

const config: ConfiguracaoDoJarvis = {
  perfil_id: "empresa", modelo: "rapido", voz_id: null, ativo: true,
  contas: [{ id: "empresa", nome: "Empresa", autenticada: true }, { id: "pessoal", nome: "Pessoal", autenticada: true }],
  modelos: [{ id: "rapido", nome: "Rápido" }, { id: "profundo", nome: "Profundo" }],
};
beforeEach(() => {
  vi.mocked(invoke).mockReset().mockImplementation(async (comando) => {
    if (comando === "jarvis_configuracao" || comando === "jarvis_configurar") return config;
    if (comando === "jarvis_vozes") return { voz: "masculina", vozes: [{ id: "masculina", nome: "Voz masculina", idioma: "pt-BR", genero: "masculino", qualidade: "aprimorada" }] };
    return null;
  });
  vi.mocked(listen).mockReset().mockResolvedValue(() => undefined);
});

it("só inicia o Jarvis depois que os eventos estão conectados", async () => {
  let conectar!: (parar: () => void) => void;
  vi.mocked(listen).mockImplementation(() => new Promise((resolve) => { conectar = resolve; }));
  render(<JanelaDoJarvis />);
  expect(invoke).not.toHaveBeenCalledWith("jarvis_iniciar");
  await act(async () => conectar(() => undefined));
  await waitFor(() => expect(invoke).toHaveBeenCalledWith("jarvis_iniciar"));
});

it("envia imagem colada sem texto e mantém os anexos até o envio ser aceito", async () => {
  render(<JanelaDoJarvis />);
  const arquivo = new File([new Uint8Array([137, 80, 78, 71, 13, 10, 26, 10])], "foto.png", { type: "image/png" });
  fireEvent.paste(screen.getByRole("textbox", { name: "Escrever para o Jarvis" }), { clipboardData: { files: [arquivo] } });
  expect(await screen.findByRole("button", { name: "Remover foto.png" })).toBeInTheDocument();
  await userEvent.click(screen.getByRole("button", { name: "Enviar para o Jarvis" }));
  await waitFor(() => expect(invoke).toHaveBeenCalledWith("jarvis_escrever", { texto: "", anexos: [{ nome: "foto.png", mime: "image/png", base64: "iVBORw0KGgo=" }] }));
  await waitFor(() => expect(screen.queryByRole("button", { name: "Remover foto.png" })).not.toBeInTheDocument());
});

it("preserva texto e documento quando o backend recusa o envio", async () => {
  vi.mocked(invoke).mockImplementation(async (comando) => {
    if (comando === "jarvis_escrever") throw "Aguarde a conexão do Jarvis";
    return null;
  });
  render(<JanelaDoJarvis />);
  fireEvent.change(screen.getByLabelText("Anexar imagem ou documento"), { target: { files: [new File(["conteúdo"], "notas.md")] } });
  await screen.findByRole("button", { name: "Remover notas.md" });
  const campo = screen.getByRole("textbox", { name: "Escrever para o Jarvis" });
  await userEvent.type(campo, "Leia as notas");
  await userEvent.click(screen.getByRole("button", { name: "Enviar para o Jarvis" }));
  expect(await screen.findByRole("alert")).toHaveTextContent("Aguarde a conexão");
  expect(campo).toHaveValue("Leia as notas");
  expect(screen.getByRole("button", { name: "Remover notas.md" })).toBeInTheDocument();
});

it("salva modelo, conta e voz escolhidos no comando de configuração", async () => {
  const salvo = vi.fn(); const fechar = vi.fn();
  render(<AjustesDoJarvis ocupado={false} onFechar={fechar} onSalvo={salvo} />);
  await userEvent.selectOptions(await screen.findByLabelText("Conta do Claude"), "pessoal");
  await userEvent.selectOptions(screen.getByLabelText("Modelo"), "profundo");
  await userEvent.selectOptions(screen.getByLabelText("Voz"), "masculina");
  await userEvent.click(screen.getByRole("button", { name: "Salvar ajustes" }));
  await waitFor(() => expect(invoke).toHaveBeenCalledWith("jarvis_configurar", { ajustes: { perfil_id: "pessoal", modelo: "profundo", voz_id: "masculina" } }));
  expect(salvo).toHaveBeenCalledWith(config);
  expect(fechar).toHaveBeenCalledOnce();
});

it("não troca o cérebro no meio de uma tarefa", async () => {
  render(<AjustesDoJarvis ocupado onFechar={vi.fn()} onSalvo={vi.fn()} />);
  expect(await screen.findByRole("button", { name: "Salvar ajustes" })).toBeDisabled();
  expect(screen.getByRole("status")).toHaveTextContent("Espere o pedido terminar");
});

it("recebe catálogo de voz sem perder a conversa e mantém o progresso da ferramenta", () => {
  const falando = aplicarEvento(estadoInicial, { tipo: "voce_disse", texto: "Consulte meus projetos" });
  const comVozes = aplicarEvento(falando, { tipo: "vozes", vozes: [], voz: null });
  const progresso = aplicarEvento(comVozes, { tipo: "ferramenta", nome: "listar_projetos", estado: "erro" });
  expect(progresso.falas[0].texto).toBe("Consulte meus projetos");
  expect(progresso.ferramenta).toEqual({ nome: "listar_projetos", estado: "erro" });
});

it("recusa formatos incompatíveis e arquivos grandes antes de carregá-los", async () => {
  await expect(prepararAnexos([new File(["x"], "programa.exe")], [])).rejects.toThrow("use uma imagem");
  const grande = new File(["x"], "grande.pdf", { type: "application/pdf" });
  Object.defineProperty(grande, "size", { value: MAXIMO_POR_ANEXO + 1 });
  await expect(prepararAnexos([grande], [])).rejects.toThrow("ultrapassa 6 MB");
});

it("limita quantidade e soma dos anexos antes de ler os arquivos", async () => {
  const pequeno = new File(["x"], "notas.txt", { type: "text/plain" });
  await expect(prepararAnexos(Array.from({ length: 7 }, () => pequeno), [])).rejects.toThrow("até 6 arquivos");
  const arquivo = new File(["x"], "foto.jpg", { type: "image/jpeg" });
  Object.defineProperty(arquivo, "size", { value: MAXIMO_POR_ANEXO });
  await expect(prepararAnexos([arquivo, arquivo, pequeno], [])).rejects.toThrow("juntos ultrapassam 12 MB");
});
