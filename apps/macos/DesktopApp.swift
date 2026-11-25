import AppKit
import Carbon.HIToolbox
import Foundation
import ServiceManagement
import SwiftUI

protocol LoginItemService {
    var status: SMAppService.Status { get }
    func register() throws
    func unregister() throws
}

@MainActor
protocol LoginItemSettings {
    static func openSystemSettingsLoginItems()
}

extension SMAppService: LoginItemService, LoginItemSettings {}

struct LoginItems {
    let service: any LoginItemService
    let settings: any LoginItemSettings.Type
    let canRegister: () -> Bool

    static let system = Self(service: SMAppService.mainApp, settings: SMAppService.self,
                             canRegister: { canRegisterBundle(at: Bundle.main.bundleURL) })

    static func canRegisterBundle(at url: URL) -> Bool {
        !url.path.contains("/AppTranslocation/") &&
            (try? url.resourceValues(forKeys: [.volumeIsReadOnlyKey]).volumeIsReadOnly) != true
    }

    @MainActor
    func toggle() throws {
        switch service.status {
        case .enabled:
            try service.unregister()
        case .requiresApproval:
            settings.openSystemSettingsLoginItems()
        default:
            guard canRegister() else {
                throw Failure("Move copied.md to Applications before enabling Launch at Login.")
            }
            try service.register()
            if service.status == .requiresApproval { settings.openSystemSettingsLoginItems() }
        }
    }
}

struct WelcomeView: View {
    let shortcut: String?
    let shortcutFailure: String?
    let copy: () -> Void
    let setShortcut: () -> Void

    var explanation: String {
        shortcut.map { "Copy Markdown in any app, then press \($0) to convert it." }
            ?? shortcutFailure
            ?? "Use the menu bar to convert Markdown from your clipboard."
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Label("copied.md", systemImage: "clipboard").font(.headline)
            Text(explanation).font(.callout).foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true)
            HStack {
                Button("Copy from Clipboard", action: copy).buttonStyle(.borderedProminent)
                if shortcut == nil {
                    Button("Set Shortcut…", action: setShortcut)
                }
            }
        }.padding(16).frame(width: 340, alignment: .leading)
    }
}

enum ShortcutCopyMode: Int, CaseIterable {
    case copyAndPick = 0
    case picker = 1
    case copyOnly = 2

    var title: String {
        switch self {
        case .copyAndPick: "Copy Last Used and Show Picker"
        case .picker: "Show Picker First"
        case .copyOnly: "Copy Last Used Only"
        }
    }
}

@MainActor
final class DesktopApp: NSObject, NSApplicationDelegate, NSMenuDelegate {
    private let runtime: any MarkdownRuntime
    private let defaults: UserDefaults
    private let clipboardDefaults: UserDefaults
    private let clipboard: () -> any ClipboardBoard
    private let chooseFormat: @MainActor ([Target], CopiedFormat?) throws -> Choice
    private let runAlert: @MainActor (NSAlert) -> NSApplication.ModalResponse
    private let makeHotKey: @MainActor () throws -> GlobalHotKey
    private let defaultShortcut: () -> Shortcut
    private let loginItems: LoginItems
    private let shortcutKey = "keyboardShortcut"
    private let disabledKey = "keyboardShortcutDisabled"
    private let shortcutModeKey = "keyboardShortcutCopyMode"
    private let firstLaunchKey = "hasLaunchedDesktopApp"
    let menu = NSMenu()
    let welcome = NSPopover()
    private(set) var statusItem: NSStatusItem?
    private var hotKey: GlobalHotKey?
    private var shortcutFailure: String?
    private var catalog: [Target]?
    private var busy = false
    private var choosing = false
    private(set) var feedback: Task<Void, Never>?

    init(
        runtime: any MarkdownRuntime = ConverterRuntime(bundleURL: Bundle.main.bundleURL),
        defaults: UserDefaults = .standard,
        clipboardDefaults: UserDefaults = preferences,
        clipboard: @escaping () -> any ClipboardBoard = generalClipboard,
        chooseFormat: (@MainActor ([Target], CopiedFormat?) throws -> Choice)? = nil,
        runAlert: @escaping @MainActor (NSAlert) -> NSApplication.ModalResponse = runDesktopAlert,
        makeHotKey: @escaping @MainActor () throws -> GlobalHotKey = { try GlobalHotKey() },
        defaultShortcut: @escaping () -> Shortcut = { Shortcut.defaultShortcut() },
        loginItems: LoginItems = .system
    ) {
        self.runtime = runtime
        self.defaults = defaults
        self.clipboardDefaults = clipboardDefaults
        self.clipboard = clipboard
        self.chooseFormat = chooseFormat ?? { try choose($0, rememberingIn: clipboardDefaults, copied: $1) }
        self.runAlert = runAlert
        self.makeHotKey = makeHotKey
        self.defaultShortcut = defaultShortcut
        self.loginItems = loginItems
    }

    private var preferredShortcut: Shortcut {
        if let data = defaults.data(forKey: shortcutKey),
           let saved = try? JSONDecoder().decode(Shortcut.self, from: data) { return saved }
        return defaultShortcut()
    }

    private var shortcutMode: ShortcutCopyMode {
        ShortcutCopyMode(rawValue: defaults.integer(forKey: shortcutModeKey)) ?? .copyAndPick
    }

    func applicationDidFinishLaunching(_ notification: Notification) {
        let item = NSStatusBar.system.statusItem(withLength: NSStatusItem.squareLength)
        statusItem = item
        menu.delegate = self
        item.menu = menu
        showStatusIcon()
        do {
            let hotKey = try prepareHotKey()
            if !defaults.bool(forKey: disabledKey) { try hotKey.bind(preferredShortcut) }
        } catch {
            shortcutFailure = error.localizedDescription
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
        if let statusItem { NSStatusBar.system.removeStatusItem(statusItem) }
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
        if shortcutMode == .picker, let shortcut = hotKey?.current { copy.title += "   " + shortcut.label }
        menu.addItem(copy)
        if let shortcutFailure { menu.addItem(item(shortcutFailure, nil)) }

        menu.addItem(.separator())
        let enabled = item("Use Keyboard Shortcut", #selector(toggleShortcut))
        enabled.state = hotKey?.current == nil ? .off : .on
        menu.addItem(enabled)
        menu.addItem(item("Change Keyboard Shortcut…", #selector(recordShortcut)))
        let behavior = item("Shortcut Behavior", nil)
        let modes = NSMenu()
        for mode in ShortcutCopyMode.allCases {
            let action = item(mode.title, #selector(setShortcutMode(_:)))
            action.tag = mode.rawValue
            action.state = mode == shortcutMode ? .on : .off
            modes.addItem(action)
        }
        behavior.submenu = modes
        menu.addItem(behavior)
        if !preferredShortcut.matches(defaultShortcut()) {
            menu.addItem(item("Reset to Default Shortcut", #selector(resetShortcut)))
        }

        menu.addItem(.separator())
        let status = loginItems.service.status
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
        welcome.contentViewController = NSHostingController(rootView: WelcomeView(
            shortcut: hotKey?.current?.label, shortcutFailure: shortcutFailure,
            copy: { [weak self] in self?.copyFromClipboard() },
            setShortcut: { [weak self] in self?.recordShortcut() }
        ))
        welcome.show(relativeTo: button.bounds, of: button, preferredEdge: .minY)
    }

    @objc private func copyFromClipboard() { startCopy() }

    @discardableResult
    func startCopy(mode: ShortcutCopyMode = .picker) -> Task<Void, Never>? {
        guard !busy else { return nil }
        busy = true
        let previousApp = NSWorkspace.shared.frontmostApplication
        welcome.performClose(nil)
        let initial: Snapshot
        do {
            initial = try snapshot(clipboard())
            guard !initial.text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
                throw Failure("The clipboard has no text. Copy Markdown and try again.")
            }
        } catch {
            present(error.localizedDescription)
            restore(previousApp)
            busy = false
            return nil
        }

        let runtime = self.runtime
        return Task {
            defer { busy = false; restore(previousApp) }
            do {
                let targets: [Target]
                if let catalog {
                    targets = catalog
                } else {
                    targets = try await Task.detached(priority: .userInitiated) { try runtime.targets() }.value
                    catalog = targets
                }
                var expectedChangeCount = initial.changeCount
                var copied: CopiedFormat?
                if mode != .picker, let target = preferredFormat(from: targets, rememberingIn: clipboardDefaults) {
                    let result = try await convertAndCopy(initial.text, to: target.id, expecting: expectedChangeCount)
                    expectedChangeCount = result.changeCount
                    copied = CopiedFormat(target: target, warning: result.warning)
                    if mode == .copyOnly {
                        presentCopyWarning(result.warning)
                        return
                    }
                }
                let selected = try pick(targets, copied: copied)
                guard let target = selected.target, target != copied?.target.id else { return }
                let result = try await convertAndCopy(initial.text, to: target, expecting: expectedChangeCount)
                presentCopyWarning(result.warning)
            } catch {
                present(error.localizedDescription)
            }
        }
    }

    private func convertAndCopy(_ markdown: String, to target: String, expecting changeCount: Int) async throws
        -> (changeCount: Int, warning: String?) {
        let result = try await Task.detached(priority: .userInitiated) { [runtime] in
            try runtime.convert(markdown, to: target)
        }.value
        let written = try write(
            WriteRequest(representations: result.representations, expectedChangeCount: changeCount, target: target),
            to: clipboard(), rememberingIn: clipboardDefaults
        )
        markCopied()
        return (written.changeCount, result.diagnostics.first { $0.severity == "warning" }?.message)
    }

    private func presentCopyWarning(_ warning: String?) {
        if let warning { present("Copied with a warning", detail: warning) }
    }

    private func pick(_ targets: [Target], copied: CopiedFormat?) throws -> Choice {
        choosing = true
        defer { choosing = false }
        return try chooseFormat(targets, copied)
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
        Self.applicationToRestore(app, frontmost: NSWorkspace.shared.frontmostApplication?.processIdentifier)?.activate(options: [])
    }

    static func applicationToRestore(_ app: NSRunningApplication?, frontmost: pid_t?) -> NSRunningApplication? {
        let ownPID = ProcessInfo.processInfo.processIdentifier
        guard let app, !app.isTerminated, app.processIdentifier != ownPID, frontmost == ownPID else { return nil }
        return app
    }

    private func prepareHotKey() throws -> GlobalHotKey {
        if let hotKey { return hotKey }
        let hotKey = try makeHotKey()
        hotKey.onPress = { [weak self] in
            guard let self else { return }
            startCopy(mode: shortcutMode)
        }
        self.hotKey = hotKey
        return hotKey
    }

    private func performBind(_ shortcut: Shortcut, remember: Bool = true) throws {
        try prepareHotKey().bind(shortcut)
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
            catch { present(error.localizedDescription) }
        }
    }

    @objc private func setShortcutMode(_ item: NSMenuItem) {
        defaults.set(item.tag, forKey: shortcutModeKey)
        refreshMenu()
    }

    @objc private func resetShortcut() {
        do {
            try performBind(defaultShortcut(), remember: false)
            defaults.removeObject(forKey: shortcutKey)
            refreshMenu()
        } catch {
            present(error.localizedDescription)
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
        _ = runAlert(alert)
        if let monitor { NSEvent.removeMonitor(monitor) }
        guard let recorded else { return }
        do { try performBind(recorded) }
        catch { present(error.localizedDescription) }
    }

    @objc private func toggleLogin() {
        do {
            try loginItems.toggle()
        } catch {
            present(error.localizedDescription)
        }
        refreshMenu()
    }

    private func present(_ title: String, detail: String = "") {
        let alert = NSAlert()
        alert.messageText = title
        alert.informativeText = detail
        alert.addButton(withTitle: "OK")
        _ = runAlert(alert)
    }
}

#if !COPIED_MD_TESTS
@main
#endif
enum CopiedDesktop {
    @MainActor
    static func main() {
        if CommandLine.arguments.dropFirst().first == "--self-test" {
            do {
                try runTests()
            } catch {
                FileHandle.standardError.write(Data((error.localizedDescription + "\n").utf8))
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
        try testTelegramPasteboard(converter)
        let app = NSApplication.shared
        app.setActivationPolicy(.prohibited)
        let hotKey = try GlobalHotKey()
        defer { hotKey.stop() }
        for key in [kVK_F18, kVK_F19] {
            let shortcut = Shortcut(keyCode: UInt32(key), modifiers: UInt32(controlKey | optionKey | shiftKey | cmdKey), keyLabel: "Test")
            try hotKey.bind(shortcut)
        }
        hotKey.unbind()
        print("Bundled converter, pasteboard formats, stale-write protection, and hotkey registration passed.")
    }

    @MainActor
    static func testTelegramPasteboard(_ converter: any MarkdownRuntime) throws {
        let markdown = "# Café 🚀\n\n- **Shipped.** See [the rollout](https://example.com/releases/1).\n- **Next.** Check `main`."
        let result = try converter.convert(markdown, to: "telegram")
        let board = NSPasteboard.withUniqueName()
        defer { board.releaseGlobally() }
        let written = try write(
            WriteRequest(representations: result.representations, expectedChangeCount: board.changeCount, target: nil),
            to: board
        )
        guard written.types == ["text/plain", "text/html", "text/rtf"],
              let rtf = board.data(forType: .rtf),
              let text = NSAttributedString(rtf: rtf, documentAttributes: nil),
              text.string == "Café 🚀\n\n• Shipped. See the rollout.\n\n• Next. Check main." else {
            throw Failure("Telegram RTF did not preserve the message layout.")
        }
        let string = text.string as NSString
        let bold = text.attribute(.font, at: string.range(of: "Shipped.").location, effectiveRange: nil) as? NSFont
        let code = text.attribute(.font, at: string.range(of: "main").location, effectiveRange: nil) as? NSFont
        let link = text.attribute(.link, at: string.range(of: "the rollout").location, effectiveRange: nil)
        guard bold?.fontDescriptor.symbolicTraits.contains(.bold) == true,
              code?.fontDescriptor.symbolicTraits.contains(.monoSpace) == true,
              link.map(String.init(describing:)) == "https://example.com/releases/1" else {
            throw Failure("Telegram RTF did not preserve emphasis, inline code, or the labeled link.")
        }
    }
}
