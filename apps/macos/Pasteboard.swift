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
    return Written(types: types)
}

@MainActor
func runDesktopAlert(_ alert: NSAlert) -> NSApplication.ModalResponse {
    activateForAlert(NSApplication.shared, cooperative: true)
    return alert.runModal()
}

@MainActor
private func runPickerAlert(_ alert: NSAlert) -> NSApplication.ModalResponse {
    let app = NSApplication.shared
    app.setActivationPolicy(.accessory)
    activateForAlert(app, cooperative: false)
    return alert.runModal()
}

@MainActor
private func activateForAlert(_ app: NSApplication, cooperative: Bool) {
    if cooperative, #available(macOS 14, *) {
        app.activate()
    } else {
        app.activate(ignoringOtherApps: true)
    }
}

@MainActor
func choose(
    _ targets: [Target], rememberingIn defaults: UserDefaults = preferences,
    runModal: @MainActor (NSAlert) -> NSApplication.ModalResponse = runPickerAlert
) throws -> Choice {
    guard !targets.isEmpty else { throw Failure("There are no available formats.") }
    let popup = NSPopUpButton(frame: NSRect(x: 0, y: 0, width: 260, height: 28))
    popup.addItems(withTitles: targets.map(\.label))
    if let recent = defaults.string(forKey: preferenceKey),
       let index = targets.firstIndex(where: { $0.id == recent }) {
        popup.selectItem(at: index)
    }
    let alert = NSAlert()
    alert.messageText = "Copy Markdown As…"
    alert.accessoryView = popup
    alert.addButton(withTitle: "Copy")
    alert.addButton(withTitle: "Cancel")
    let confirmed = runModal(alert) == .alertFirstButtonReturn
    let index = popup.indexOfSelectedItem
    return Choice(target: confirmed && targets.indices.contains(index) ? targets[index].id : nil)
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
