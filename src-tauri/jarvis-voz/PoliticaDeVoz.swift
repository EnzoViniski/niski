// Fork: catálogo e escolha de vozes locais do Jarvis. Não instala vozes, não pede microfone.
import AVFoundation
import Foundation

struct VozLocal: Equatable {
    let id: String
    let nome: String
    let idioma: String
    let qualidade: String
    let genero: String
    var expressiva = false

    var metadados: [String: Any] {
        ["id": id, "nome": nome, "idioma": idioma, "qualidade": qualidade, "genero": genero]
    }

    private var nivel: Int {
        switch qualidade { case "premium": return 3; case "aprimorada": return 2; default: return 1 }
    }

    /// Mantém o timbre masculino antes de comparar qualidade; não altera o tom de uma voz feminina.
    static func preferida(entre vozes: [VozLocal]) -> VozLocal? {
        vozes.filter { $0.idioma.lowercased().replacingOccurrences(of: "_", with: "-").hasPrefix("pt-") }
            .sorted {
                // Vozes de personagens soam artificiais; continuam disponíveis para escolha manual.
                if $0.expressiva != $1.expressiva { return !$0.expressiva }
                if ($0.genero == "masculino") != ($1.genero == "masculino") { return $0.genero == "masculino" }
                if ($0.idioma == "pt-BR") != ($1.idioma == "pt-BR") { return $0.idioma == "pt-BR" }
                if $0.nivel != $1.nivel { return $0.nivel > $1.nivel }
                return $0.id < $1.id
            }.first
    }
}

enum VozesDoJarvis {
    static func disponiveis() -> [AVSpeechSynthesisVoice] {
        // A lista da Apple contém as vozes disponíveis no sistema, inclusive pacotes já baixados.
        AVSpeechSynthesisVoice.speechVoices().filter { $0.language.lowercased().hasPrefix("pt-") }
    }

    static func descrever(_ voz: AVSpeechSynthesisVoice) -> VozLocal {
        let qualidade: String
        switch voz.quality { case .premium: qualidade = "premium"; case .enhanced: qualidade = "aprimorada"; default: qualidade = "padrao" }
        let genero: String
        switch voz.gender { case .male: genero = "masculino"; case .female: genero = "feminino"; default: genero = "nao_informado" }
        return VozLocal(id: voz.identifier, nome: voz.name, idioma: voz.language, qualidade: qualidade,
                        genero: genero, expressiva: voz.voiceTraits.contains(.isNoveltyVoice) || voz.identifier.hasPrefix("com.apple.eloquence."))
    }

    static func padrao() -> AVSpeechSynthesisVoice? {
        let vozes = disponiveis()
        guard let escolhida = VozLocal.preferida(entre: vozes.map(descrever)) else { return nil }
        return vozes.first { $0.identifier == escolhida.id }
    }

    static func catalogo(escolhida: AVSpeechSynthesisVoice?) -> [String: Any] {
        let vozes = disponiveis().map(descrever).sorted {
            if $0.nome != $1.nome { return $0.nome.localizedStandardCompare($1.nome) == .orderedAscending }
            return $0.id < $1.id
        }
        return ["vozes": vozes.map(\.metadados), "voz": escolhida.map { $0.identifier as Any } ?? NSNull()]
    }
}

/// Comandos curtos de interrupção devem funcionar mesmo enquanto ainda existe cauda de eco.
func pedeParaCalar(_ texto: String) -> Bool {
    let normalizado = texto.lowercased().folding(options: .diacriticInsensitive, locale: nil)
        .components(separatedBy: CharacterSet.alphanumerics.inverted).filter { !$0.isEmpty }.joined(separator: " ")
    return ["pare", "parar", "cancele", "cancelar", "silencio", "jarvis pare", "jarvis parar", "jarvis silencio"].contains(normalizado)
}

/// Testes sem áudio: preferência masculina/qualidade, fallback e comandos de interrupção.
func verificarPoliticaDeVoz() -> Bool {
    let basica = VozLocal(id: "masculina-basica", nome: "M", idioma: "pt-BR", qualidade: "padrao", genero: "masculino")
    let aprimorada = VozLocal(id: "masculina-aprimorada", nome: "M", idioma: "pt-BR", qualidade: "aprimorada", genero: "masculino")
    let premium = VozLocal(id: "feminina-premium", nome: "F", idioma: "pt-BR", qualidade: "premium", genero: "feminino")
    let inglesa = VozLocal(id: "inglesa", nome: "E", idioma: "en-US", qualidade: "premium", genero: "masculino")
    let portuguesa = VozLocal(id: "portuguesa", nome: "P", idioma: "pt-PT", qualidade: "premium", genero: "masculino")
    let personagem = VozLocal(id: "personagem", nome: "P", idioma: "pt-BR", qualidade: "premium", genero: "masculino", expressiva: true)
    return VozLocal.preferida(entre: [premium, basica, aprimorada, inglesa]) == aprimorada
        && VozLocal.preferida(entre: [premium]) == premium
        && VozLocal.preferida(entre: [personagem, premium]) == premium
        && VozLocal.preferida(entre: [premium, portuguesa]) == portuguesa
        && VozLocal.preferida(entre: [basica, portuguesa]) == basica
        && VozLocal.preferida(entre: [inglesa]) == nil
        && VozLocal.preferida(entre: []) == nil
        && pedeParaCalar("Jarvis, silêncio!") && pedeParaCalar("Pare.")
        && !pedeParaCalar("Não precisa parar")
}
