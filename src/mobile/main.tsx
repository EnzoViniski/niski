import { createRoot } from "react-dom/client";
import { MobileApp } from "./MobileApp";
import "../index.css";
import "./pagina.css";
import "./mobile.css";
// Fork: estilo Minimalista, quando o PC usa ele (`tema.ts` põe o data-estilo). Por último, como no PC.
import "../styles/minimalista.css";

createRoot(document.getElementById("root")!).render(<MobileApp />);
