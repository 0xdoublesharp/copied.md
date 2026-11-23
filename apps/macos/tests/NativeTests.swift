import AppKit
import Carbon.HIToolbox
import Foundation
import ServiceManagement
import SwiftUI

private func expect(_ condition: Bool, _ message: String) throws {
    if !condition { throw Failure(message) }
}

private func expectFailure(_ message: String, code: Int32 = 1, _ perform: () throws -> Void) throws {
    do {
        try perform()
    } catch let error as Failure {
        try expect(error.message.contains(message) && error.exitCode == code,
                   "Expected \(message) with exit code \(code), got \(error.message) (\(error.exitCode)).")
        return
    }
    throw Failure("Expected failure: \(message)")
}

private func pasteboardInvariants() throws {
    let board = NSPasteboard.withUniqueName()
    defer { board.releaseGlobally() }
    try expectFailure("no text") { _ = try snapshot(board) }
    let values = ["**Ready**", "👩🏽‍💻 🛰️", "e\u{301}\nsecond line", "tabs\t0", "end \\ { }"]
    for (index, value) in values.enumerated() {
        let expectCount = board.changeCount
        let html = index.isMultiple(of: 2) ? "<strong>Ready</strong>" : ""
        let rtf = index.isMultiple(of: 2) ? "{\\rtf1 Ready}" : ""
        let written = try write(WriteRequest(
            representations: ["text/plain": value, "text/html": html, "text/rtf": rtf],
            expectedChangeCount: expectCount, target: nil
        ), to: board)
        let captured = try snapshot(board)
        try expect(captured.text == value, "Plain text changed during an isolated pasteboard write.")
        try expect(captured.changeCount != expectCount, "A successful write must change the generation.")
        if html.isEmpty {
            try expect(written.types == ["text/plain"], "An empty rich flavor must be omitted.")
            try expect(board.string(forType: .html) == nil && board.data(forType: .rtf) == nil,
                       "A prior write's rich flavors must not leak into plain text.")
        } else {
            try expect(written.types == ["text/plain", "text/html", "text/rtf"], "Missing rich flavors.")
            try expect(board.string(forType: .html) == html, "HTML did not survive the pasteboard write.")
            try expect(board.data(forType: .rtf) == Data(rtf.utf8), "RTF did not survive the pasteboard write.")
        }
        try expectFailure("changed", code: 3) {
            _ = try write(WriteRequest(
                representations: ["text/plain": "stale"],
                expectedChangeCount: expectCount, target: nil
            ), to: board)
        }
        for representations in [["text/plain": " \n\t"], ["text/html": "<b>Ready</b>"]] {
            try expectFailure("no text") {
                _ = try write(WriteRequest(
                    representations: representations, expectedChangeCount: nil, target: nil
                ), to: board)
            }
        }
        let unchanged = try snapshot(board)
        try expect(unchanged.text == captured.text && unchanged.changeCount == captured.changeCount,
                   "A rejected write changed the board.")
    }
}

private final class InterleavedPasteboard: ClipboardBoard {
    var changeCount = 0
    var text = "Original"
    var copyDuringRead = false
    var copyAfterClear = false
    var rejectWrite = false
    var writeCalls = 0
    var html: String?
    var frozenRevision = false
    var corruptSecondWrite = false
    var omitHTML = false

    func string(forType type: NSPasteboard.PasteboardType) -> String? {
        if type == .html { return omitHTML ? nil : html }
        let captured = text
        if copyDuringRead {
            changeCount += 1
            text = "New external text"
        }
        return captured
    }

    func clearContents() -> Int {
        if !frozenRevision { changeCount += 1 }
        text = ""
        let prepared = changeCount
        if copyAfterClear {
            changeCount += 1
            text = "New external text"
        }
        return prepared
    }

    func writeObjects(_ objects: [any NSPasteboardWriting]) -> Bool {
        writeCalls += 1
        if rejectWrite { return false }
        let item = objects.first as? NSPasteboardItem
        text = corruptSecondWrite && writeCalls == 2 ? "wrong" : item?.string(forType: .string) ?? ""
        html = item?.string(forType: .html)
        return true
    }
}

private final class RejectingPasteboardItem: NSPasteboardItem {
    override func setString(_ string: String, forType type: NSPasteboard.PasteboardType) -> Bool {
        type != .html && super.setString(string, forType: type)
    }
    override func setData(_ data: Data, forType type: NSPasteboard.PasteboardType) -> Bool {
        type != .rtf && super.setData(data, forType: type)
    }
}

private func pasteboardFailures() throws {
    let board = InterleavedPasteboard()
    for flavor in ["text/html", "text/rtf"] {
        try expectFailure("could not be prepared") {
            let request = WriteRequest(representations: ["text/plain": "Example", flavor: "Example"],
                                       expectedChangeCount: 0, target: nil)
            _ = try write(request, to: board, makeItem: RejectingPasteboardItem.init)
        }
    }
    try expect(board.writeCalls == 0 && board.changeCount == 0, "Pasteboard changed before its flavors were prepared.")
    board.corruptSecondWrite = true
    try expectFailure("changed the pasteboard") { try checkPasteboard(board) }
    let frozen = InterleavedPasteboard()
    frozen.frozenRevision = true
    try expectFailure("stale clipboard write was accepted") { try checkPasteboard(frozen) }
    let corrupt = InterleavedPasteboard()
    corrupt.omitHTML = true
    try expectFailure("did not round-trip") { try checkPasteboard(corrupt) }
    try expect((generalClipboard() as? NSPasteboard)?.name == .general, "The helper must address the shared board.")
    try expect(clipboardPreferences(nil) === UserDefaults.standard, "Without suite preferences, use the standard defaults.")
}

private func clipboardInterleavings() throws {
    let board = InterleavedPasteboard()
    board.copyDuringRead = true
    try expectFailure("changed", code: 3) { _ = try snapshot(board) }

    board.copyAfterClear = true
    let request = WriteRequest(representations: ["text/plain": "Converted"],
                               expectedChangeCount: board.changeCount, target: nil)
    try expectFailure("changed", code: 3) { _ = try write(request, to: board) }
    try expect(board.text == "New external text", "A late external copy was overwritten.")
    try expect(board.writeCalls == 0, "A changed clipboard was written after its revision was observed.")

    board.copyAfterClear = false
    board.rejectWrite = true
    try expectFailure("rejected") {
        _ = try write(WriteRequest(representations: ["text/plain": "Converted"],
                                   expectedChangeCount: nil, target: nil), to: board)
    }
}

@MainActor
private func shortcutInvariants() throws {
    func event(_ flags: NSEvent.ModifierFlags, _ characters: String) -> NSEvent {
        NSEvent.keyEvent(
            with: .keyDown, location: .zero, modifierFlags: flags, timestamp: 0,
            windowNumber: 0, context: nil, characters: characters,
            charactersIgnoringModifiers: characters, isARepeat: false, keyCode: UInt16(kVK_ANSI_M)
        )!
    }
    let defaultShortcut = Shortcut.defaultShortcut()
    try expect(defaultShortcut.label == "⌃⌥⌘M", "Unexpected default shortcut.")
    try expect(Shortcut.defaultShortcut { nil }.keyCode == UInt32(kVK_ANSI_M), "The fallback key must be ANSI M.")
    try expect(Shortcut.findM { _ in nil } == nil, "A layout without M must fall back to the ANSI key.")
    try expect(Shortcut.findM { $0 == 17 ? "M" : "N" } == 17, "The keyboard layout's M mapping was ignored.")
    try expectFailure("already in use") {
        try GlobalHotKey.checkRegistration(OSStatus(eventHotKeyExistsErr), receivedHandle: false, shortcut: defaultShortcut)
    }
    try expectFailure("could not register") {
        try GlobalHotKey.checkRegistration(noErr, receivedHandle: false, shortcut: defaultShortcut)
    }
    for flags: NSEvent.ModifierFlags in [[], [.option, .shift], [.command], [.control]] {
        try expect(Shortcut.record(event(flags, "m")) == nil,
                   "Accepts a chord without enough qualifying modifiers.")
    }
    for characters in ["!", "", "mn", "ß"] {
        try expect(Shortcut.record(event([.control, .option], characters)) == nil,
                   "Accepts a chord without one alphanumeric key.")
    }
    let shortcut = Shortcut.record(event([.control, .option, .shift, .command], "m"))
    try expect(shortcut?.label == "⌃⌥⇧⌘M", "Modifiers are missing or out of order.")
    guard let shortcut else { throw Failure("No recorded shortcut.") }
    let roundTrip = try JSONDecoder().decode(Shortcut.self, from: JSONEncoder().encode(shortcut))
    try expect(roundTrip == shortcut, "Saved shortcut did not round-trip.")
    try expect(shortcut.matches(Shortcut(keyCode: shortcut.keyCode, modifiers: shortcut.modifiers, keyLabel: "Z")),
               "Shortcut identity must use physical key and modifiers.")
    try expect(!shortcut.matches(defaultShortcut), "Different modifier sets were treated as equal.")
}

@MainActor
private func hotKeyEvents() async throws {
    let hotKey = try GlobalHotKey()
    defer { hotKey.stop() }
    let shortcut = Shortcut(keyCode: UInt32(kVK_F18), modifiers: UInt32(controlKey | optionKey | shiftKey | cmdKey), keyLabel: "Test")
    try hotKey.bind(shortcut)
    try hotKey.bind(shortcut)

    var event: EventRef?
    let created = CreateEvent(nil, OSType(kEventClassKeyboard), UInt32(kEventHotKeyPressed),
                              GetCurrentEventTime(), 0, &event)
    guard created == noErr, let event else { throw Failure("Could not create a Carbon test event.") }
    defer { ReleaseEvent(event) }
    let owner = Unmanaged.passUnretained(hotKey).toOpaque()
    var callbacks = 0
    hotKey.onPress = { callbacks += 1 }
    try expect(GlobalHotKey.handleEvent(nil, nil, owner) == eventNotHandledErr,
               "The hotkey handler accepted a missing event.")
    try expect(GlobalHotKey.handleEvent(nil, event, owner) == eventNotHandledErr,
               "The hotkey handler accepted a missing key parameter.")
    func send(signature: OSType, id: UInt32) throws -> OSStatus {
        var key = EventHotKeyID(signature: signature, id: id)
        let status = SetEventParameter(
            event, EventParamName(kEventParamDirectObject), EventParamType(typeEventHotKeyID),
            MemoryLayout<EventHotKeyID>.size, &key
        )
        try expect(status == noErr, "Could not attach the test key to the Carbon event.")
        return GlobalHotKey.handleEvent(nil, event, owner)
    }

    let unrelated = try send(signature: 0x41424344, id: 1)
    try expect(unrelated == eventNotHandledErr, "The hotkey handler accepted another application's signature.")
    _ = try send(signature: 0x434D4443, id: 0)
    _ = try send(signature: 0x434D4443, id: 1)
    await withCheckedContinuation { continuation in
        DispatchQueue.main.async { continuation.resume() }
    }
    try expect(callbacks == 1, "Only the currently registered hotkey should invoke the copy action.")

    hotKey.unbind()
    _ = try send(signature: 0x434D4443, id: 1)
    await withCheckedContinuation { continuation in
        DispatchQueue.main.async { continuation.resume() }
    }
    try expect(callbacks == 1, "An unregistered shortcut invoked the copy action.")
}

@MainActor
private func afterModalStarts(_ action: @escaping @MainActor () -> Void) {
    let timer = Timer(timeInterval: 0.05, repeats: false) { _ in
        MainActor.assumeIsolated { action() }
    }
    RunLoop.main.add(timer, forMode: .modalPanel)
}

@MainActor
private func pickerInvariants(_ defaults: UserDefaults) throws {
    let targets = [Target(id: "plain-text", label: "Plain Text"), Target(id: "telegram", label: "Telegram")]
    defaults.set("telegram", forKey: preferenceKey)
    let chosen = try choose(targets, rememberingIn: defaults) { alert in
        let picker = alert.accessoryView as? NSPopUpButton
        precondition(picker?.indexOfSelectedItem == 1)
        return .alertFirstButtonReturn
    }
    try expect(chosen.target == "telegram", "Picker did not restore the most recent target.")
    defaults.set("removed-target", forKey: preferenceKey)
    let cancelled = try choose(targets, rememberingIn: defaults) { alert in
        precondition((alert.accessoryView as? NSPopUpButton)?.indexOfSelectedItem == 0)
        return .alertSecondButtonReturn
    }
    try expect(cancelled.target == nil, "A cancelled picker supplied a target.")

    afterModalStarts { NSApp.stopModal(withCode: .alertSecondButtonReturn) }
    let dismissed = try choose(targets, rememberingIn: defaults)
    try expect(dismissed.target == nil, "A real modal dismissal selected a target.")
    afterModalStarts { NSApp.stopModal(withCode: .alertFirstButtonReturn) }
    let response = runDesktopAlert(NSAlert())
    try expect(response == .alertFirstButtonReturn, "A desktop alert did not return its modal response.")
    NSApp.setActivationPolicy(.prohibited)
}

@MainActor
private func helperInvariants() throws {
    let board = NSPasteboard.withUniqueName()
    defer { board.releaseGlobally() }
    let request = Data("""
      {"representations":{"text/plain":"From helper","text/html":"<strong>From helper</strong>"}}
      """.utf8)
    try PasteboardCommand.run("write", clipboard: { board }, input: { request })
    try expect(board.string(forType: .string) == "From helper", "Helper did not write plain text.")
    try expect(board.string(forType: .html) == "<strong>From helper</strong>", "Helper did not write HTML.")
    try PasteboardCommand.run("read", clipboard: { board })
    let unchanged = board.changeCount
    let input = Data(#"[{"id":"plain-text","label":"Plain Text"}]"#.utf8)
    try PasteboardCommand.run("choose", clipboard: { board }, input: { input }) { targets in
        try expect(targets.map(\.id) == ["plain-text"], "Helper did not read the supplied formats.")
        return Choice(target: nil)
    }
    try expect(board.changeCount == unchanged, "Reading or cancelling the helper changed the board.")
}

private struct TestRuntime: MarkdownRuntime {
    let warning: Bool
    var failCatalog = false
    var failConversion = false
    var representations: [String: String]? = nil

    func targets() throws -> [Target] {
        if failCatalog { throw Failure("Catalog unavailable") }
        return [Target(id: "plain-text", label: "Plain Text")]
    }

    func convert(_ text: String, to target: String) throws -> DesktopConversion {
        if failConversion { throw Failure("Conversion unavailable") }
        return DesktopConversion(schemaVersion: 1, target: target,
                                 representations: representations ?? ["text/plain": "Converted: " + text],
                                 diagnostics: warning ? [.init(severity: "warning", message: "Example warning")] : [])
    }
}

@MainActor
private func telegramSelfTestFailures() throws {
    try expectFailure("did not preserve the message layout") {
        try CopiedDesktop.testTelegramPasteboard(TestRuntime(warning: false))
    }
    let unstyled = #"{\rtf1\ansi\uc1 Caf\u233? \u-10179?\u-8576?\par\par\u8226? Shipped. See the rollout.\par\par\u8226? Next. Check main.}"#
    try expectFailure("did not preserve emphasis") {
        try CopiedDesktop.testTelegramPasteboard(TestRuntime(warning: false, representations: [
            "text/plain": "Example", "text/html": "<p>Example</p>", "text/rtf": unstyled,
        ]))
    }
}

@MainActor
private func desktopCopyInvariants(_ defaults: UserDefaults) async throws {
    let board = InterleavedPasteboard()
    var target: String? = "plain-text"
    var writeExternalTextBeforeConversion = false
    var alerts: [(String, String)] = []
    let desktop = DesktopApp(
        runtime: TestRuntime(warning: true), defaults: defaults, clipboardDefaults: defaults,
        clipboard: { board }, chooseFormat: { formats in
            try expect(formats.map(\.id) == ["plain-text"], "Desktop provided the wrong formats.")
            if writeExternalTextBeforeConversion {
                board.changeCount += 1
                board.text = "New external text"
            }
            return Choice(target: target)
        },
        runAlert: { alert in
            alerts.append((alert.messageText, alert.informativeText))
            return .alertFirstButtonReturn
        }
    )
    let termination = Notification(name: NSApplication.willTerminateNotification)
    defaults.set(true, forKey: "hasLaunchedDesktopApp")
    defaults.set(true, forKey: "keyboardShortcutDisabled")
    desktop.applicationDidFinishLaunching(Notification(name: NSApplication.didFinishLaunchingNotification))
    defer { desktop.applicationWillTerminate(termination) }

    let first = desktop.startCopy()
    try expect(first != nil && desktop.startCopy() == nil, "Overlapping conversions were accepted.")
    await first?.value
    try expect(board.text == "Converted: Original", "Desktop did not write the conversion.")
    try expect(defaults.string(forKey: preferenceKey) == "plain-text", "Successful copy did not remember the target.")
    try expect(alerts.last?.0 == "Copied with a warning" && alerts.last?.1 == "Example warning",
               "A conversion warning was lost.")
    try expect(desktop.statusItem?.button?.toolTip == "copied.md copied the result", "Copy did not show success feedback.")
    await desktop.feedback?.value
    try expect(desktop.statusItem?.button?.toolTip == "copied.md", "Feedback did not restore the normal status icon.")

    target = nil
    await desktop.startCopy()?.value
    try expect(board.writeCalls == 1, "A cancelled picker wrote a conversion.")

    target = "plain-text"
    writeExternalTextBeforeConversion = true
    await desktop.startCopy()?.value
    try expect(board.text == "New external text" && board.writeCalls == 1,
               "The app overwrote text copied while the picker was open.")
    try expect(alerts.last?.0.contains("clipboard changed") == true, "A stale write did not explain the failure.")

    writeExternalTextBeforeConversion = false
    board.copyAfterClear = true
    defaults.set("rich-text", forKey: preferenceKey)
    let alertCount = alerts.count
    await desktop.startCopy()?.value
    try expect(board.text == "New external text" && board.writeCalls == 1,
               "The app overwrote a copy that arrived after clearing.")
    try expect(alerts.count == alertCount + 1 && alerts.last?.0.contains("clipboard changed") == true,
               "A late clipboard change did not report a single error.")
    try expect(defaults.string(forKey: preferenceKey) == "rich-text",
               "An unsuccessful write updated the remembered format.")
    board.copyAfterClear = false

    board.text = " \n\t "
    try expect(desktop.startCopy() == nil, "Whitespace advanced to the picker.")
    try expect(alerts.last?.0.contains("no text") == true, "Empty clipboard was not reported.")

    board.text = "Markdown"
    let unavailable = DesktopApp(
        runtime: TestRuntime(warning: false, failCatalog: true), defaults: defaults,
        clipboardDefaults: defaults, clipboard: { board }, chooseFormat: { _ in
            throw Failure("Picker should not open without a catalog.")
        },
        runAlert: { alert in
            alerts.append((alert.messageText, alert.informativeText))
            return .alertFirstButtonReturn
        }
    )
    await unavailable.startCopy()?.value
    try expect(alerts.last?.0 == "Catalog unavailable", "An unavailable converter did not reach the user.")
    try expect(board.writeCalls == 1, "An unavailable converter changed the clipboard.")

    let conversionFailure = DesktopApp(
        runtime: TestRuntime(warning: false, failConversion: true), defaults: defaults,
        clipboardDefaults: defaults, clipboard: { board }, chooseFormat: { _ in Choice(target: "plain-text") },
        runAlert: { alert in
            alerts.append((alert.messageText, alert.informativeText))
            return .alertFirstButtonReturn
        }
    )
    let unchanged = board.changeCount
    await conversionFailure.startCopy()?.value
    try expect(alerts.last?.0 == "Conversion unavailable", "A conversion error did not reach the user.")
    try expect(board.text == "Markdown" && board.changeCount == unchanged && board.writeCalls == 1,
               "A conversion error changed the clipboard.")

    let defaultPicker = DesktopApp(runtime: TestRuntime(warning: false), defaults: defaults,
                                   clipboardDefaults: defaults, clipboard: { board })
    afterModalStarts {
        defaultPicker.applicationDidResignActive(Notification(name: NSApplication.didResignActiveNotification))
    }
    await defaultPicker.startCopy()?.value
    try expect(board.changeCount == unchanged, "Losing focus in the picker wrote to the clipboard.")
}

private final class TestLoginService: LoginItemService {
    var status = SMAppService.Status.notRegistered
    var newRegistrationNeedsApproval = true
    var registrations = 0
    var removals = 0

    func register() throws {
        registrations += 1
        status = newRegistrationNeedsApproval ? .requiresApproval : .enabled
    }

    func unregister() throws {
        removals += 1
        status = .notRegistered
    }
}

private enum TestLoginSettings: LoginItemSettings {
    static var opens = 0
    static func openSystemSettingsLoginItems() { opens += 1 }
}

@MainActor
private func desktopLifecycle(_ defaults: UserDefaults) throws {
    let app = NSApplication.shared
    app.setActivationPolicy(.prohibited)
    defaults.removeObject(forKey: "hasLaunchedDesktopApp")
    defaults.removeObject(forKey: "keyboardShortcutDisabled")
    let shortcut = Shortcut(keyCode: UInt32(kVK_F18), modifiers: UInt32(controlKey | optionKey | shiftKey | cmdKey), keyLabel: "F18")
    let fallback = Shortcut(keyCode: UInt32(kVK_F19), modifiers: shortcut.modifiers, keyLabel: "F19")
    defaults.set(try JSONEncoder().encode(shortcut), forKey: "keyboardShortcut")
    let startup = Notification(name: NSApplication.didFinishLaunchingNotification)
    let termination = Notification(name: NSApplication.willTerminateNotification)
    var alerts: [String] = []
    var canRegister = true
    let service = TestLoginService()
    TestLoginSettings.opens = 0
    let login = LoginItems(service: service, settings: TestLoginSettings.self, canRegister: { canRegister })
    let desktop = DesktopApp(
        defaults: defaults,
        clipboard: {
            let empty = InterleavedPasteboard()
            empty.text = ""
            return empty
        },
        runAlert: { alert in alerts.append(alert.messageText); return .alertSecondButtonReturn },
        makeHotKey: { throw Failure("Application test shortcut unavailable") },
        defaultShortcut: { fallback },
        loginItems: login
    )
    try expect(!desktop.applicationShouldHandleReopen(app, hasVisibleWindows: false), "Reopening must use the menu bar.")
    desktop.applicationDidFinishLaunching(startup)
    defer { desktop.applicationWillTerminate(termination) }
    try expect(defaults.bool(forKey: "hasLaunchedDesktopApp"), "The welcome must be remembered after the first launch.")
    guard let welcome = desktop.welcome.contentViewController as? NSHostingController<WelcomeView> else {
        throw Failure("The first launch did not build the welcome popover.")
    }
    let noShortcut = WelcomeView(shortcut: nil, shortcutFailure: nil, copy: {}, setShortcut: {})
    try expect(noShortcut.explanation.contains("Use the menu bar"), "Missing welcome guidance without a shortcut.")
    _ = welcome.rootView.body
    welcome.rootView.setShortcut()
    welcome.rootView.copy()
    try expect(alerts.last?.contains("no text") == true, "The welcome's copy action did not read the clipboard fixture.")
    desktop.menuWillOpen(desktop.menu)
    desktop.applicationDidResignActive(Notification(name: NSApplication.didResignActiveNotification))
    try expect(desktop.menu.items.contains { $0.title == "Application test shortcut unavailable" },
               "Menus did not report the unavailable hotkey.")
    try expect(desktop.menu.item(withTitle: "Use Keyboard Shortcut")?.state == .off,
               "A failed registration appeared enabled.")
    try expect(desktop.menu.item(withTitle: "Quit copied.md")?.keyEquivalent == "q",
               "The desktop menu is missing its quit shortcut.")

    func select(_ title: String, in desktop: DesktopApp) throws {
        guard let item = desktop.menu.item(withTitle: title), let action = item.action else {
            throw Failure("Missing desktop menu action \(title).")
        }
        try expect(app.sendAction(action, to: item.target, from: item), "Menu action did not reach the desktop.")
    }
    try select("Use Keyboard Shortcut", in: desktop)
    try expect(alerts.last == "Application test shortcut unavailable", "Failed enable did not explain the error.")
    try select("Reset to Default Shortcut", in: desktop)
    try expect(defaults.data(forKey: "keyboardShortcut") != nil, "A failed reset erased the saved shortcut.")

    try select("Launch at Login", in: desktop)
    try expect(service.registrations == 1 && TestLoginSettings.opens == 1, "Registration requiring approval did not open Settings.")
    try expect(desktop.menu.item(withTitle: "Approve Launch at Login…")?.state == .mixed, "Pending approval was not shown.")
    try select("Approve Launch at Login…", in: desktop)
    try expect(service.registrations == 1 && TestLoginSettings.opens == 2, "Approving a registration tried to register again.")
    service.status = .enabled
    desktop.menuWillOpen(desktop.menu)
    try expect(desktop.menu.item(withTitle: "Launch at Login")?.state == .on, "Enabled login item appeared off.")
    try select("Launch at Login", in: desktop)
    try expect(service.removals == 1 && service.status == .notRegistered, "Disabling launch at login did not unregister.")
    canRegister = false
    try select("Launch at Login", in: desktop)
    try expect(alerts.last?.contains("Move copied.md to Applications") == true && service.registrations == 1,
               "A read-only copy was allowed to register.")
    service.newRegistrationNeedsApproval = false
    canRegister = true
    try select("Launch at Login", in: desktop)
    try expect(service.registrations == 2 && TestLoginSettings.opens == 2, "An approved login item unnecessarily opened Settings.")
    try expect(!LoginItems.canRegisterBundle(at: URL(fileURLWithPath: "/private/AppTranslocation/Instance/copied.md.app")),
               "A translocated app was offered login registration.")
    try expect(LoginItems.canRegisterBundle(at: Bundle.main.bundleURL),
               "The writable test bundle was refused.")

    _ = LoginItems.system.canRegister()
    var sendRecorderEvents = false
    var installed: GlobalHotKey?
    let working = DesktopApp(defaults: defaults, clipboard: {
        let empty = InterleavedPasteboard()
        empty.text = ""
        return empty
    }, runAlert: { alert in
        if !sendRecorderEvents { return .alertSecondButtonReturn }
        afterModalStarts {
            let windowNumber = alert.window.windowNumber
            for (keyCode, label, modifiers, repeating): (Int, String, NSEvent.ModifierFlags, Bool) in [
                (kVK_ANSI_X, "x", [], false),
                (kVK_ANSI_X, "x", [.control, .option, .shift, .command], true),
                (kVK_ANSI_X, "x", [.control, .option, .shift, .command], false),
            ] {
                let event = NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: modifiers,
                                             timestamp: 0, windowNumber: windowNumber, context: nil,
                                             characters: label, charactersIgnoringModifiers: label,
                                             isARepeat: repeating, keyCode: UInt16(keyCode))!
                app.postEvent(event, atStart: false)
            }
            let timeout = Timer(timeInterval: 0.5, repeats: false) { _ in
                MainActor.assumeIsolated { if app.modalWindow != nil { app.abortModal() } }
            }
            RunLoop.main.add(timeout, forMode: .modalPanel)
        }
        return alert.runModal()
    }, makeHotKey: {
        let hotKey = try GlobalHotKey()
        installed = hotKey
        return hotKey
    }, defaultShortcut: { fallback })
    working.applicationDidFinishLaunching(startup)
    defer { working.applicationWillTerminate(termination) }
    try expect(working.menu.item(withTitle: "Use Keyboard Shortcut")?.state == .on, "Saved shortcut was not enabled.")
    installed?.onPress?()
    try expect(working.menu.item(withTitle: "Reset to Default Shortcut") != nil, "Custom shortcut cannot be reset.")
    try select("About copied.md", in: working)
    guard let workingWelcome = working.welcome.contentViewController as? NSHostingController<WelcomeView> else {
        throw Failure("The About menu did not create a welcome popover.")
    }
    try expect(workingWelcome.rootView.shortcut == shortcut.label, "About omitted the active shortcut.")
    _ = workingWelcome.rootView.body
    try select("Change Keyboard Shortcut…", in: working)
    try expect(defaults.data(forKey: "keyboardShortcut") != nil, "Cancelling the recorder erased the shortcut.")
    try select("Use Keyboard Shortcut", in: working)
    try expect(defaults.bool(forKey: "keyboardShortcutDisabled"), "Disabling the shortcut was not persisted.")
    try expect(working.menu.item(withTitle: "Use Keyboard Shortcut")?.state == .off, "Disabled shortcut still appeared enabled.")
    try select("Use Keyboard Shortcut", in: working)
    try expect(!defaults.bool(forKey: "keyboardShortcutDisabled"), "Re-enabling the shortcut was not persisted.")
    try expect(working.menu.item(withTitle: "Use Keyboard Shortcut")?.state == .on, "Re-enabled shortcut appeared disabled.")
    sendRecorderEvents = true
    try select("Change Keyboard Shortcut…", in: working)
    let recorded = try JSONDecoder().decode(Shortcut.self, from: defaults.data(forKey: "keyboardShortcut") ?? Data())
    try expect(recorded.keyLabel == "X", "The recorder did not save a valid local key event.")
    try select("Reset to Default Shortcut", in: working)
    try expect(defaults.data(forKey: "keyboardShortcut") == nil && working.menu.item(withTitle: "Reset to Default Shortcut") == nil,
               "Reset did not restore the default shortcut.")

    let ownPID = ProcessInfo.processInfo.processIdentifier
    if let other = NSWorkspace.shared.runningApplications.first(where: { !$0.isTerminated && $0.processIdentifier != ownPID }) {
        var restored: pid_t?
        DesktopApp.restoreFocus(to: other, frontmost: nil) { restored = $0.processIdentifier }
        try expect(restored == nil, "Restoring focus would interrupt another active application.")
        DesktopApp.restoreFocus(to: other, frontmost: ownPID) { restored = $0.processIdentifier }
        try expect(restored == other.processIdentifier, "The previous application did not regain focus.")
    }
    DesktopApp.reactivate(.current)
}

private func runtimeInvariants(_ bundle: URL, node: URL, converter: URL) throws {
    let files = FileManager.default
    let helpers = bundle.appendingPathComponent("Contents/Helpers")
    let resources = bundle.appendingPathComponent("Contents/Resources")
    for path in [helpers, resources] {
        try files.createDirectory(at: path, withIntermediateDirectories: true)
    }
    try files.createSymbolicLink(at: helpers.appendingPathComponent("node"), withDestinationURL: node)
    let entrypoint = resources.appendingPathComponent("converter.mjs")
    try files.copyItem(at: converter, to: entrypoint)
    let runtime = ConverterRuntime(bundleURL: bundle)
    let catalog = try runtime.targets()
    try expect(catalog.contains { $0.id == "telegram" }, "Bundled converter omitted Telegram.")
    let result = try runtime.convert("**Ready** [Notes](https://example.org/notes)", to: "telegram")
    try expect(result.representations["text/plain"] == "Ready Notes (https://example.org/notes)",
               "Bundled conversion produced incorrect text.")
    try expect(result.representations["text/rtf"]?.contains("HYPERLINK") == true,
               "Bundled conversion omitted the RTF link.")
    let board = NSPasteboard.withUniqueName()
    defer { board.releaseGlobally() }
    _ = try write(WriteRequest(representations: result.representations, expectedChangeCount: nil, target: nil),
                  to: board)
    guard let rtf = board.data(forType: .rtf),
          let rich = NSAttributedString(rtf: rtf, documentAttributes: nil) else {
        throw Failure("AppKit could not decode the converter's RTF.")
    }
    try expect(rich.string == "Ready Notes", "AppKit changed the RTF's visible label.")
    let range = (rich.string as NSString).range(of: "Notes")
    let link = rich.attribute(.link, at: range.location, effectiveRange: nil)
    try expect(String(describing: link ?? "") == "https://example.org/notes", "AppKit dropped the link.")
    let large = String(repeating: "Abcd😀 ", count: 25000)
    let largeResult = try runtime.convert(large, to: "plain-text")
    try expect(largeResult.representations["text/plain"] == large.trimmingCharacters(in: .whitespaces),
               "A large conversion lost data over the subprocess pipes.")
    try expectFailure("could not convert") { _ = try runtime.convert("Example", to: "missing") }
    try expectFailure("could not convert") { _ = try runtime.convert(large, to: "missing") }
    try expectFailure("could not start") {
        _ = try ConverterRuntime(bundleURL: bundle.appendingPathComponent("missing")).targets()
    }
    try "console.log('[]')".write(to: entrypoint, atomically: true, encoding: .utf8)
    try expectFailure("no available copy formats") { _ = try runtime.targets() }
    for (version, target) in [(2, "telegram"), (1, "plain-text")] {
        try """
            console.log(JSON.stringify({schemaVersion:\(version),target:'\(target)',
              representations:{'text/plain':'Example'},diagnostics:[]}))
            """.write(to: entrypoint, atomically: true, encoding: .utf8)
        try expectFailure("invalid conversion result") { _ = try runtime.convert("Example", to: "telegram") }
    }
}

@main
enum NativeTests {
    @MainActor static func main() async {
        do {
            if CommandLine.arguments.dropFirst().first == "launch" {
                try testApplicationLaunch()
                return
            }
            try await run()
        } catch {
            let message = (error as? Failure)?.message ?? error.localizedDescription
            FileHandle.standardError.write(Data(("Native tests failed: " + message + "\n").utf8))
            exit(1)
        }
    }

    @MainActor private static func testApplicationLaunch() throws {
        let app = NSApplication.shared
        let defaults = UserDefaults.standard
        try expect(defaults.bool(forKey: "hasLaunchedDesktopApp") && defaults.bool(forKey: "keyboardShortcutDisabled"),
                   "The isolated launch requires first-run UI and global hotkeys to be disabled.")
        var launched = false
        let stop = Timer(timeInterval: 0.1, repeats: false) { _ in
            MainActor.assumeIsolated {
                let desktop = app.delegate as? DesktopApp
                launched = desktop?.menu.item(withTitle: "Copy Markdown As…") != nil && app.activationPolicy() == .accessory
                desktop?.applicationWillTerminate(Notification(name: NSApplication.willTerminateNotification))
                app.stop(nil)
                if let wake = NSEvent.otherEvent(with: .applicationDefined, location: .zero, modifierFlags: [], timestamp: 0,
                                                 windowNumber: 0, context: nil, subtype: 0, data1: 0, data2: 0) {
                    app.postEvent(wake, atStart: false)
                }
            }
        }
        RunLoop.main.add(stop, forMode: .default)
        CopiedDesktop.main()
        try expect(launched, "The native entry point did not launch a menu-bar app.")
    }

    @MainActor private static func run() async throws {
        let args = CommandLine.arguments
        guard args.count == 4 else { throw Failure("Usage: NativeTests BUNDLE_DIR NODE CONVERTER") }
        try pasteboardInvariants()
        try pasteboardFailures()
        try clipboardInterleavings()
        try expectFailure("no available formats") { _ = try choose([]) }
        try shortcutInvariants()
        _ = NSApplication.shared
        let suiteName = "md.copied.test.\(UUID().uuidString)"
        guard let defaults = UserDefaults(suiteName: suiteName) else { throw Failure("No isolated preferences.") }
        defer { defaults.removePersistentDomain(forName: suiteName) }
        try await hotKeyEvents()
        try runtimeInvariants(
            URL(fileURLWithPath: args[1]), node: URL(fileURLWithPath: args[2]),
            converter: URL(fileURLWithPath: args[3])
        )
        try pickerInvariants(defaults)
        try helperInvariants()
        try telegramSelfTestFailures()
        try await desktopCopyInvariants(defaults)
        try desktopLifecycle(defaults)
        print("Native clipboard, picker, desktop, hotkey and converter runtime tests passed.")
    }
}
