import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { TitleBar } from "../components/TitleBar";
import { NOME_DO_PRODUTO } from "../lib/identidade";

const janela = { minimize: vi.fn(async () => undefined), toggleMaximize: vi.fn(async () => undefined), close: vi.fn(async () => undefined),
  isMaximized: vi.fn(async () => false), onResized: vi.fn(async () => () => undefined) };
vi.mock("@tauri-apps/api/window", () => ({ getCurrentWindow: () => janela }));

it("barra superior mostra onde você está e controla a janela", async () => {
  const onHome = vi.fn();
  render(<TitleBar trilha={["Workspace 1", "omni-agents"]} dica="D:/omni-agents" onHome={onHome} onOpenSettings={() => undefined} />);
  expect(screen.getByText("omni-agents")).toHaveAttribute("title", "D:/omni-agents");
  expect(document.querySelector("header")).toHaveAttribute("data-tauri-drag-region");

  await userEvent.click(screen.getByRole("button", { name: "Minimizar" }));
  await userEvent.click(screen.getByRole("button", { name: "Maximizar" }));
  await userEvent.click(screen.getByRole("button", { name: "Fechar" }));
  await vi.waitFor(() => expect(janela.close).toHaveBeenCalled());
  expect(janela.minimize).toHaveBeenCalled();
  expect(janela.toggleMaximize).toHaveBeenCalled();

  await userEvent.click(screen.getByRole("button", { name: new RegExp(NOME_DO_PRODUTO) }));
  expect(onHome).toHaveBeenCalled();
});
