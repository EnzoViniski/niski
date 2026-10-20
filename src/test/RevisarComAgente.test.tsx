import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "./interacoesTelas";
import { RevisarComAgente } from "../features/revisao/RevisarComAgente";
import { revisarComAgente, mandarAchados } from "../features/revisao/revisaoPorAgenteService";

vi.mock("../features/revisao/revisaoPorAgenteService",() => ({revisarComAgente:vi.fn(),mandarAchados:vi.fn()}));
const revisao = { id:"filha",session_id:"s-filha",name:"Revisão · Autora",origem:"mae" };
beforeEach(() => { sessionStorage.clear(); vi.clearAllMocks(); vi.mocked(revisarComAgente).mockResolvedValue(revisao); vi.mocked(mandarAchados).mockResolvedValue(); });
it("abre outro agente e encaminha achados à origem", async () => {
  const abrir = vi.fn();
  render(<RevisarComAgente sessionId="s-mae" onAbrirSessao={abrir} />);
  await userEvent.click(screen.getByRole("button",{name:"Revisar com agente"}));
  await userEvent.click(screen.getByRole("button",{name:"Iniciar revisão"}));
  await waitFor(() => expect(abrir).toHaveBeenCalledWith("s-filha","Revisão · Autora"));
  expect(revisarComAgente).toHaveBeenCalledWith("s-mae",{provider:"claude",escopo:"local"},expect.any(String));
  await userEvent.click(screen.getByRole("button",{name:"Mandar achados para a origem"}));
  await waitFor(() => expect(mandarAchados).toHaveBeenCalledWith("s-mae",revisao,expect.any(String)));
});
it("escolhe CLI e exige base antes de comparar branches", async () => {
  render(<RevisarComAgente sessionId="s" />);
  await userEvent.click(screen.getByRole("button",{name:"Revisar com agente"}));
  await userEvent.selectOptions(screen.getByLabelText("CLI da revisão"),"codex");
  await userEvent.selectOptions(screen.getByLabelText("Escopo do agente"),"branch");
  expect(screen.getByRole("button",{name:"Iniciar revisão"})).toBeDisabled();
  await userEvent.type(screen.getByLabelText("Branch base"),"origin/main");
  await userEvent.click(screen.getByRole("button",{name:"Iniciar revisão"}));
  await waitFor(() => expect(revisarComAgente).toHaveBeenCalledWith("s",{provider:"codex",escopo:"branch",base:"origin/main"},expect.any(String)));
});
it("repete a mesma chave após erro e mostra a falha", async () => {
  vi.mocked(revisarComAgente).mockRejectedValueOnce(new Error("Limite de agentes"));
  render(<RevisarComAgente sessionId="s" />);
  await userEvent.click(screen.getByRole("button",{name:"Revisar com agente"}));
  await userEvent.click(screen.getByRole("button",{name:"Iniciar revisão"}));
  expect(await screen.findByRole("alert")).toHaveTextContent("Limite de agentes");
  await userEvent.click(screen.getByRole("button",{name:"Iniciar revisão"}));
  await waitFor(() => expect(revisarComAgente).toHaveBeenCalledTimes(2));
  expect(vi.mocked(revisarComAgente).mock.calls[0][2]).toBe(vi.mocked(revisarComAgente).mock.calls[1][2]);
});
it("sem origem mantém o botão indisponível", () => { render(<RevisarComAgente />); expect(screen.getByRole("button",{name:"Revisar com agente"})).toBeDisabled(); });

it("conserva o revisor quando abrir a aba desmonta o painel", async () => {
  const painel = render(<RevisarComAgente sessionId="s-mae" onAbrirSessao={vi.fn()} />);
  await userEvent.click(screen.getByRole("button",{name:"Revisar com agente"}));
  await userEvent.click(screen.getByRole("button",{name:"Iniciar revisão"}));
  await screen.findByRole("button",{name:"Mandar achados para a origem"});
  painel.unmount();
  render(<RevisarComAgente sessionId="s-mae" />);
  await userEvent.click(screen.getByRole("button",{name:"Mandar achados para a origem"}));
  await waitFor(() => expect(mandarAchados).toHaveBeenCalledWith("s-mae",revisao,expect.any(String)));
});
