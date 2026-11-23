import AppKit
import Foundation

@main
enum PasteboardCommand {
    @MainActor
    static func main() {
        do {
            switch CommandLine.arguments.dropFirst().first {
            case "read":
                try respond(snapshot(.general))
            case "write":
                try respond(write(request(WriteRequest.self), to: .general))
            case "choose":
                try respond(choose(request([Target].self)))
            case "self-test":
                try selfTest()
            default:
                throw Failure("Usage: copied-md-pasteboard read|write|choose|self-test", exitCode: 2)
            }
        } catch {
            let failure = error as? Failure
            let message = failure?.message ?? "The clipboard request was invalid."
            FileHandle.standardError.write(Data((message + "\n").utf8))
            exit(failure?.exitCode ?? 1)
        }
    }
}
