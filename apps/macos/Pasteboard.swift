import AppKit
import Foundation

struct Failure: Error {
    let message: String
    let exitCode: Int32
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
}

struct Target: Decodable {
    let id: String
    let label: String
}

struct Choice: Encodable {
    let target: String?
}

let preferenceKey = "copied-md.lastTarget"
let preferences = UserDefaults(suiteName: "md.copied.clipboard") ?? .standard

func respond<T: Encodable>(_ value: T) throws {
    let data = try JSONEncoder().encode(value)
    FileHandle.standardOutput.write(data + Data([10]))
}

func request<T: Decodable>(_ type: T.Type) throws -> T {
    try JSONDecoder().decode(type, from: FileHandle.standardInput.readDataToEndOfFile())
}

func snapshot(_ pasteboard: NSPasteboard) throws -> Snapshot {
    let changeCount = pasteboard.changeCount
    guard let text = pasteboard.string(forType: .string) else {
        throw Failure("The clipboard has no text.")
    }
    guard changeCount == pasteboard.changeCount else {
        throw Failure("The clipboard changed. Copy the Markdown again.", exitCode: 3)
    }
    return Snapshot(text: text, changeCount: changeCount)
}

func write(_ request: WriteRequest, to pasteboard: NSPasteboard) throws -> Written {
    let item = NSPasteboardItem()
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

    if let expected = request.expectedChangeCount, expected != pasteboard.changeCount {
        throw Failure("The clipboard changed. Copy the Markdown again.", exitCode: 3)
    }
    pasteboard.clearContents()
    guard pasteboard.writeObjects([item]) else {
        throw Failure("macOS rejected the clipboard write.")
    }
    if let target = request.target { preferences.set(target, forKey: preferenceKey) }
    return Written(types: types)
}

@MainActor
func choose(_ targets: [Target]) throws -> Choice {
    guard !targets.isEmpty else { throw Failure("There are no available formats.") }
    let app = NSApplication.shared
    app.setActivationPolicy(.accessory)
    let popup = NSPopUpButton(frame: NSRect(x: 0, y: 0, width: 260, height: 28))
    popup.addItems(withTitles: targets.map(\.label))
    if let recent = preferences.string(forKey: preferenceKey),
       let index = targets.firstIndex(where: { $0.id == recent }) {
        popup.selectItem(at: index)
    }
    let alert = NSAlert()
    alert.messageText = "Copy Markdown As…"
    alert.accessoryView = popup
    alert.addButton(withTitle: "Copy")
    alert.addButton(withTitle: "Cancel")
    app.activate(ignoringOtherApps: true)
    let confirmed = alert.runModal() == .alertFirstButtonReturn
    let index = popup.indexOfSelectedItem
    return Choice(target: confirmed && targets.indices.contains(index) ? targets[index].id : nil)
}

func selfTest() throws {
    let board = NSPasteboard.withUniqueName()
    defer { board.releaseGlobally() }
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
