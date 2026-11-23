import Foundation
import Darwin

struct ConversionDiagnostic: Decodable {
    let severity: String
    let message: String
}

struct DesktopConversion: Decodable {
    let schemaVersion: Int
    let target: String
    let representations: [String: String]
    let diagnostics: [ConversionDiagnostic]
}

protocol MarkdownRuntime: Sendable {
    func targets() throws -> [Target]
    func convert(_ text: String, to target: String) throws -> DesktopConversion
}

struct ConverterRuntime: MarkdownRuntime {
    let bundleURL: URL

    func targets() throws -> [Target] {
        let targets = try run(["targets", "--json"], input: nil, as: [Target].self)
        guard !targets.isEmpty else { throw Failure("There are no available copy formats.") }
        return targets
    }

    func convert(_ text: String, to target: String) throws -> DesktopConversion {
        let result = try run(["convert", "--to", target, "--json"], input: Data(text.utf8), as: DesktopConversion.self)
        guard result.schemaVersion == 1, result.target == target else {
            throw Failure("copied.md received an invalid conversion result.")
        }
        return result
    }

    private func run<T: Decodable>(_ arguments: [String], input: Data?, as type: T.Type) throws -> T {
        let process = Process()
        process.executableURL = bundleURL.appendingPathComponent("Contents/Helpers/node")
        process.arguments = [bundleURL.appendingPathComponent("Contents/Resources/converter.mjs").path] + arguments
        process.environment = ["PATH": "/usr/bin:/bin", "LANG": "en_US.UTF-8"]
        process.standardError = FileHandle.nullDevice

        let output = Pipe()
        let incoming = Pipe()
        process.standardOutput = output
        process.standardInput = incoming
        // A converter that exits before reading stdin must not terminate the parent.
        _ = fcntl(incoming.fileHandleForWriting.fileDescriptor, F_SETNOSIGPIPE, 1)

        do {
            try process.run()
        } catch {
            throw Failure("The converter could not start. Reinstall copied.md and try again.")
        }

        // Feed input concurrently so large conversions cannot block on a full output pipe.
        DispatchQueue.global(qos: .userInitiated).async {
            let handle = incoming.fileHandleForWriting
            if let input { try? handle.write(contentsOf: input) }
            try? handle.close()
        }
        let data = output.fileHandleForReading.readDataToEndOfFile()
        process.waitUntilExit()

        guard process.terminationStatus == 0,
              let result = try? JSONDecoder().decode(type, from: data) else {
            throw Failure("copied.md could not convert this clipboard text.")
        }
        return result
    }
}
