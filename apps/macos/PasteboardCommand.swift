import AppKit
import Foundation

#if !COPIED_MD_TESTS
@main
#endif
enum PasteboardCommand {
    @MainActor
    static func main() {
        do {
            try run(CommandLine.arguments.dropFirst().first)
        } catch {
            let failure = error as? Failure
            let message = failure?.message ?? "The clipboard request was invalid."
            FileHandle.standardError.write(Data((message + "\n").utf8))
            exit(failure?.exitCode ?? 1)
        }
    }

    @MainActor
    static func run(
        _ command: String?,
        clipboard: () -> any ClipboardBoard = generalClipboard,
        input: () -> Data = { FileHandle.standardInput.readDataToEndOfFile() },
        picker: @MainActor ([Target]) throws -> Choice = { try choose($0) }
    ) throws {
        switch command {
        case "read":
            try respond(snapshot(clipboard()))
        case "write":
            let request = try JSONDecoder().decode(WriteRequest.self, from: input())
            try respond(write(request, to: clipboard()))
        case "choose":
            let targets = try JSONDecoder().decode([Target].self, from: input())
            try respond(picker(targets))
        case "self-test":
            try selfTest()
        default:
            throw Failure("Usage: copied-md-pasteboard read|write|choose|self-test", exitCode: 2)
        }
    }
}
