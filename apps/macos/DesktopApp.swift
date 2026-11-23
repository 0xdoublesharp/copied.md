import AppKit
import Carbon.HIToolbox
import Foundation
import ServiceManagement
import SwiftUI

@MainActor
final class DesktopApp: NSObject, NSApplicationDelegate, NSMenuDelegate {
    private let runtime = ConverterRuntime(bundleURL: Bundle.main.bundleURL)
    private let defaults = UserDefaults.standard
    private let shortcutKey = "keyboardShortcut"
    private let disabledKey = "keyboardShortcutDisabled"
    private let firstLaunchKey = "hasLaunchedDesktopApp"
    private let menu = NSMenu()
    private let welcome = NSPopover()
    private var statusItem: NSStatusItem?
    private var hotKey: GlobalHotKey?
    private var shortcutFailure: String?
    private var catalog: [Target]?
    private var busy = false
    private var choosing = false
    private var feedback: Task<Void, Never>?

    private var preferredShortcut: Shortcut {
        if let data = defaults.data(forKey: shortcutKey),
           let saved = try? JSONDecoder().decode(Shortcut.self, from: data) { return saved }
        return .defaultShortcut()
    }

    func applicationDidFinishLaunching(_ notification: Notification) {
        let item = NSStatusBar.system.statusItem(withLength: NSStatusItem.squareLength)
        statusItem = item
        menu.delegate = self
        item.menu = menu
        showStatusIcon()
        do {
            let hotKey = try GlobalHotKey()
            hotKey.onPress = { [weak self] in self?.copyFromClipboard() }
            self.hotKey = hotKey
            if !defaults.bool(forKey: disabledKey) { try hotKey.bind(preferredShortcut) }
        } catch {
            shortcutFailure = message(for: error)
        }
        refreshMenu()
        if !defaults.bool(forKey: firstLaunchKey) {
            defaults.set(true, forKey: firstLaunchKey)
            showWelcome()
        }
    }

    func applicationShouldHandleReopen(_ sender: NSApplication, hasVisibleWindows: Bool) -> Bool {
        if !busy { showWelcome() }
        return false
    }

    func applicationDidResignActive(_ notification: Notification) {
        if choosing, NSApp.modalWindow != nil { NSApp.abortModal() }
    }

    func applicationWillTerminate(_ notification: Notification) {
        feedback?.cancel()
        hotKey?.stop()
    }

    func menuWillOpen(_ menu: NSMenu) { refreshMenu() }

    private func item(_ title: String, _ selector: Selector?) -> NSMenuItem {
        let entry = NSMenuItem(title: title, action: selector, keyEquivalent: "")
        entry.target = self
        return entry
    }

    private func refreshMenu() {
        menu.removeAllItems()
        let copy = item("Copy Markdown As…", #selector(copyFromClipboard))
        if let shortcut = hotKey?.current { copy.title += "   " + shortcut.label }
        menu.addItem(copy)
        if let shortcutFailure { menu.addItem(item(shortcutFailure, nil)) }

        menu.addItem(.separator())
        let enabled = item("Use Keyboard Shortcut", #selector(toggleShortcut))
        enabled.state = hotKey?.current == nil ? .off : .on
        menu.addItem(enabled)
        menu.addItem(item("Change Keyboard Shortcut…", #selector(recordShortcut)))
        if !preferredShortcut.matches(.defaultShortcut()) {
            menu.addItem(item("Reset to Default Shortcut", #selector(resetShortcut)))
        }

        menu.addItem(.separator())
        let status = SMAppService.mainApp.status
        let login = item(status == .requiresApproval ? "Approve Launch at Login…" : "Launch at Login", #selector(toggleLogin))
        login.state = status == .enabled ? .on : (status == .requiresApproval ? .mixed : .off)
        menu.addItem(login)
        menu.addItem(item("About copied.md", #selector(showWelcome)))
        let quit = item("Quit copied.md", #selector(NSApplication.terminate(_:)))
        quit.target = NSApp
        quit.keyEquivalent = "q"
        menu.addItem(.separator())
        menu.addItem(quit)
    }

    private func showStatusIcon(highlight: Bool = false) {
        let symbol = highlight ? "checkmark.circle" : "clipboard"
        let label = highlight ? "copied.md copied the result" : "copied.md"
        let button = statusItem?.button
        button?.image = NSImage(systemSymbolName: symbol, accessibilityDescription: label)
        button?.image?.isTemplate = true
        button?.toolTip = label
    }

    @objc private func showWelcome() {
        guard let button = statusItem?.button else { return }
        if welcome.isShown { welcome.performClose(nil) }
        welcome.behavior = .transient
        let shortcut = hotKey?.current?.label
        let explanation = shortcut.map { "Copy Markdown in any app, then press \($0) to choose a format." }
            ?? shortcutFailure
            ?? "Use the menu bar to convert Markdown from your clipboard."
        welcome.contentViewController = NSHostingController(rootView:
            VStack(alignment: .leading, spacing: 12) {
                Label("copied.md", systemImage: "clipboard")
                    .font(.headline)
                Text(explanation).font(.callout).foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true)
                HStack {
                    Button("Copy from Clipboard") { [weak self] in self?.copyFromClipboard() }
                        .buttonStyle(.borderedProminent)
                    if shortcut == nil {
                        Button("Set Shortcut…") { [weak self] in self?.recordShortcut() }
                    }
                }
            }.padding(16).frame(width: 340, alignment: .leading)
        )
        welcome.show(relativeTo: button.bounds, of: button, preferredEdge: .minY)
    }

    @objc private func copyFromClipboard() {
        guard !busy else { return }
        busy = true
        let previousApp = NSWorkspace.shared.frontmostApplication
        welcome.performClose(nil)
        let initial: Snapshot
        do {
            initial = try snapshot(.general)
            guard !initial.text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
                throw Failure("The clipboard has no text. Copy Markdown and try again.")
            }
        } catch {
            present(message(for: error))
            restore(previousApp)
            busy = false
            return
        }

        let runtime = self.runtime
        Task {
            defer { busy = false; restore(previousApp) }
            do {
                let targets: [Target]
                if let catalog {
                    targets = catalog
                } else {
                    targets = try await Task.detached(priority: .userInitiated) { try runtime.targets() }.value
                    catalog = targets
                }
                let selected = try pick(targets)
                guard let target = selected.target else { return }
                let result = try await Task.detached(priority: .userInitiated) {
                    try runtime.convert(initial.text, to: target)
                }.value
                _ = try write(
                    WriteRequest(representations: result.representations, expectedChangeCount: initial.changeCount, target: target),
                    to: .general
                )
                markCopied()
                if let warning = result.diagnostics.first(where: { $0.severity == "warning" }) {
                    present("Copied with a warning", detail: warning.message)
                }
            } catch {
                present(message(for: error))
            }
        }
    }

    private func pick(_ targets: [Target]) throws -> Choice {
        choosing = true
        defer { choosing = false }
        return try choose(targets)
    }

    private func markCopied() {
        feedback?.cancel()
        showStatusIcon(highlight: true)
        feedback = Task {
            try? await Task.sleep(nanoseconds: 1_500_000_000)
            if !Task.isCancelled { showStatusIcon() }
        }
    }

    private func restore(_ app: NSRunningApplication?) {
        guard let app, !app.isTerminated,
              app.processIdentifier != ProcessInfo.processInfo.processIdentifier,
              NSWorkspace.shared.frontmostApplication?.processIdentifier == ProcessInfo.processInfo.processIdentifier else { return }
        app.activate(options: [])
    }

    private func performBind(_ shortcut: Shortcut, remember: Bool = true) throws {
        if hotKey == nil {
            let hotKey = try GlobalHotKey()
            hotKey.onPress = { [weak self] in self?.copyFromClipboard() }
            self.hotKey = hotKey
        }
        try hotKey?.bind(shortcut)
        if remember { defaults.set(try JSONEncoder().encode(shortcut), forKey: shortcutKey) }
        defaults.set(false, forKey: disabledKey)
        shortcutFailure = nil
        refreshMenu()
    }

    @objc private func toggleShortcut() {
        if hotKey?.current != nil {
            hotKey?.unbind()
            defaults.set(true, forKey: disabledKey)
            shortcutFailure = nil
            refreshMenu()
        } else {
            do { try performBind(preferredShortcut, remember: false) }
            catch { present(message(for: error)) }
        }
    }

    @objc private func resetShortcut() {
        do {
            try performBind(.defaultShortcut(), remember: false)
            defaults.removeObject(forKey: shortcutKey)
            refreshMenu()
        } catch {
            present(message(for: error))
        }
    }

    @objc private func recordShortcut() {
        guard !busy else { return }
        busy = true
        defer { busy = false }
        let previousApp = NSWorkspace.shared.frontmostApplication
        defer { restore(previousApp) }
        welcome.performClose(nil)
        let alert = NSAlert()
        alert.messageText = "Change Keyboard Shortcut"
        alert.informativeText = "Press a letter or number with at least two modifiers, including Control or Command."
        alert.addButton(withTitle: "Cancel").keyEquivalent = "\u{1b}"
        var recorded: Shortcut?
        let monitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { event in
            if event.keyCode == 53 { return event } // Escape
            guard !event.isARepeat, let value = Shortcut.record(event) else {
                NSSound.beep()
                return nil
            }
            recorded = value
            NSApp.stopModal(withCode: .alertFirstButtonReturn)
            return nil
        }
        activateApp()
        _ = alert.runModal()
        if let monitor { NSEvent.removeMonitor(monitor) }
        guard let recorded else { return }
        do { try performBind(recorded) }
        catch { present(message(for: error)) }
    }

    @objc private func toggleLogin() {
        let service = SMAppService.mainApp
        do {
            switch service.status {
            case .enabled:
                try service.unregister()
            case .requiresApproval:
                SMAppService.openSystemSettingsLoginItems()
            default:
                let url = Bundle.main.bundleURL
                if url.path.contains("/AppTranslocation/") ||
                    (try? url.resourceValues(forKeys: [.volumeIsReadOnlyKey]).volumeIsReadOnly) == true {
                    throw Failure("Move copied.md to Applications before enabling Launch at Login.")
                }
                try service.register()
                if service.status == .requiresApproval { SMAppService.openSystemSettingsLoginItems() }
            }
        } catch {
            present(message(for: error))
        }
        refreshMenu()
    }

    private func message(for error: Error) -> String {
        (error as? Failure)?.message ?? error.localizedDescription
    }

    private func present(_ title: String, detail: String = "") {
        let alert = NSAlert()
        alert.messageText = title
        alert.informativeText = detail
        alert.addButton(withTitle: "OK")
        activateApp()
        _ = alert.runModal()
    }

    private func activateApp() {
        if #available(macOS 14, *) { NSApp.activate() }
        else { NSApp.activate(ignoringOtherApps: true) }
    }
}

@main
enum CopiedDesktop {
    @MainActor
    static func main() {
        if CommandLine.arguments.dropFirst().first == "--self-test" {
            do {
                try runTests()
            } catch {
                let description = (error as? Failure)?.message ?? error.localizedDescription
                FileHandle.standardError.write(Data((description + "\n").utf8))
                exit(1)
            }
            return
        }
        let app = NSApplication.shared
        app.setActivationPolicy(.accessory)
        let delegate = DesktopApp()
        app.delegate = delegate
        withExtendedLifetime(delegate) { app.run() }
    }

    @MainActor
    private static func runTests() throws {
        let converter = ConverterRuntime(bundleURL: Bundle.main.bundleURL)
        let targets = try converter.targets()
        guard targets.contains(where: { $0.id == "rich-text" }) else {
            throw Failure("The bundled converter did not list Rich Text.")
        }
        let result = try converter.convert("# Café\n\n[Example][ref] **bold**.\n\n[ref]: https://example.com", to: "rich-text")
        guard result.representations["text/plain"] == "Café\n\nExample (https://example.com) bold.",
              result.representations["text/html"]?.contains("<a href=\"https://example.com\">Example</a> <strong>bold</strong>") == true,
              result.diagnostics.isEmpty else {
            throw Failure("The bundled converter did not preserve Markdown or URLs.")
        }
        try selfTest()
        let app = NSApplication.shared
        app.setActivationPolicy(.prohibited)
        let hotKey = try GlobalHotKey()
        defer { hotKey.stop() }
        for key in [kVK_F18, kVK_F19] {
            let shortcut = Shortcut(keyCode: UInt32(key), modifiers: UInt32(controlKey | optionKey | shiftKey | cmdKey), keyLabel: "Test")
            try hotKey.bind(shortcut)
            guard hotKey.current == shortcut else { throw Failure("Hotkey registration was lost.") }
        }
        hotKey.unbind()
        guard hotKey.current == nil else { throw Failure("Hotkey registration was not released.") }
        print("Bundled converter, pasteboard formats, stale-write protection, and hotkey registration passed.")
    }
}
