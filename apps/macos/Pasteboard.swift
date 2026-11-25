import AppKit
import Foundation

protocol ClipboardBoard {
    var changeCount: Int { get }
    func string(forType type: NSPasteboard.PasteboardType) -> String?
    @discardableResult func clearContents() -> Int
    func writeObjects(_ objects: [any NSPasteboardWriting]) -> Bool
}

extension NSPasteboard: ClipboardBoard {}

struct Failure: LocalizedError {
    let message: String
    let exitCode: Int32
    var errorDescription: String? { message }
    init(_ message: String, exitCode: Int32 = 1) {
        self.message = message
        self.exitCode = exitCode
    }
}

struct Snapshot: Encodable {
    let text: String
    let changeCount: Int
}

struct WriteRequest: Decodable {
    let representations: [String: String]
    let expectedChangeCount: Int?
    let target: String?
}

struct Written: Encodable {
    let types: [String]
    let changeCount: Int

    private enum CodingKeys: String, CodingKey { case types }
}

struct Target: Decodable {
    let id: String
    let label: String
}

struct Choice: Encodable {
    let target: String?
}

let preferenceKey = "copied-md.lastTarget"
let preferences = clipboardPreferences(UserDefaults(suiteName: "md.copied.clipboard"))
let chromiumCustomData = NSPasteboard.PasteboardType("org.chromium.web-custom-data")

func clipboardPreferences(_ suite: UserDefaults?) -> UserDefaults { suite ?? .standard }

func generalClipboard() -> any ClipboardBoard { NSPasteboard.general }

func respond<T: Encodable>(_ value: T) throws {
    let data = try JSONEncoder().encode(value)
    FileHandle.standardOutput.write(data + Data([10]))
}

func snapshot(_ pasteboard: any ClipboardBoard) throws -> Snapshot {
    let changeCount = pasteboard.changeCount
    guard let text = pasteboard.string(forType: .string) else {
        throw Failure("The clipboard has no text.")
    }
    guard changeCount == pasteboard.changeCount else {
        throw Failure("The clipboard changed. Copy the Markdown again.", exitCode: 3)
    }
    return Snapshot(text: text, changeCount: changeCount)
}

func write(
    _ request: WriteRequest, to pasteboard: any ClipboardBoard, rememberingIn defaults: UserDefaults = preferences,
    makeItem: () -> NSPasteboardItem = NSPasteboardItem.init
) throws -> Written {
    let item = makeItem()
    guard let plain = request.representations["text/plain"],
          !plain.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty,
          item.setString(plain, forType: .string) else {
        throw Failure("There is no text to copy.")
    }

    var types = ["text/plain"]
    if let html = request.representations["text/html"], !html.isEmpty {
        guard item.setString(html, forType: .html) else {
            throw Failure("The copy could not be prepared.")
        }
        types.append("text/html")
    }
    if let rtf = request.representations["text/rtf"], !rtf.isEmpty {
        guard item.setData(Data(rtf.utf8), forType: .rtf) else {
            throw Failure("The copy could not be prepared.")
        }
        types.append("text/rtf")
    }
    if let signal = request.representations["text/signal"], !signal.isEmpty {
        guard item.setData(signalPasteboardData(signal), forType: chromiumCustomData) else {
            throw Failure("The copy could not be prepared.")
        }
        types.append("text/signal")
    }

    if let expected = request.expectedChangeCount, expected != pasteboard.changeCount {
        throw Failure("The clipboard changed. Copy the Markdown again.", exitCode: 3)
    }
    let prepared = pasteboard.clearContents()
    guard prepared == pasteboard.changeCount else {
        throw Failure("The clipboard changed. Copy the Markdown again.", exitCode: 3)
    }
    guard pasteboard.writeObjects([item]) else {
        throw Failure("macOS rejected the clipboard write.")
    }
    if let target = request.target { defaults.set(target, forKey: preferenceKey) }
    return Written(types: types, changeCount: prepared)
}

private func signalPasteboardData(_ html: String) -> Data {
    // Chromium DataTransfer custom data uses a Pickle with UTF-16 strings aligned to four bytes.
    func appendUInt32(_ value: UInt32, to data: inout Data) {
        withUnsafeBytes(of: value.littleEndian) { data.append(contentsOf: $0) }
    }

    var payload = Data()
    appendUInt32(1, to: &payload)
    for value in ["text/signal", html] {
        let codeUnits = value.utf16.map { $0.littleEndian }
        appendUInt32(UInt32(codeUnits.count), to: &payload)
        codeUnits.withUnsafeBytes { payload.append(contentsOf: $0) }
        payload.append(Data(repeating: 0, count: (4 - payload.count % 4) % 4))
    }
    var result = Data()
    appendUInt32(UInt32(payload.count), to: &result)
    result.append(payload)
    return result
}

@MainActor
func runDesktopAlert(_ alert: NSAlert) -> NSApplication.ModalResponse {
    activateForAlert(NSApplication.shared, cooperative: true)
    return alert.runModal()
}

@MainActor
func activateForAlert(_ app: NSApplication, cooperative: Bool) {
    if cooperative, #available(macOS 14, *) {
        app.activate()
    } else {
        app.activate(ignoringOtherApps: true)
    }
}

func selfTest() throws {
    let board = NSPasteboard.withUniqueName()
    defer { board.releaseGlobally() }
    try checkPasteboard(board)
}

func checkPasteboard(_ board: any ClipboardBoard) throws {
    let first = WriteRequest(
        representations: ["text/plain": "# Example", "text/html": "<h1>Example</h1>"],
        expectedChangeCount: board.changeCount,
        target: nil
    )
    let result = try write(first, to: board)
    let captured = try snapshot(board)
    guard result.types == ["text/plain", "text/html"],
          captured.text == "# Example",
          board.string(forType: .html) == "<h1>Example</h1>" else {
        throw Failure("Pasteboard types did not round-trip.")
    }
    _ = try write(WriteRequest(representations: ["text/plain": "new"], expectedChangeCount: captured.changeCount, target: nil), to: board)
    do {
        _ = try write(first, to: board)
        throw Failure("A stale clipboard write was accepted.")
    } catch let failure as Failure where failure.exitCode == 3 {
        guard try snapshot(board).text == "new" else {
            throw Failure("A stale clipboard write changed the pasteboard.")
        }
    }
    try respond(result)
}
