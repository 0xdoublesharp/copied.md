import AppKit

struct CopiedFormat {
    let target: Target
    let warning: String?
}

private let hiddenFormatsKey = "copied-md.pickerHiddenFormats"
private let shownFormatsKey = "copied-md.pickerShownFormats"

private func hiddenFormats(in defaults: UserDefaults, for targets: [Target]) -> Set<String> {
    let shown = Set(defaults.stringArray(forKey: shownFormatsKey) ?? [])
    let hiddenByDefault = targets.filter { $0.enabledByDefault == false && !shown.contains($0.id) }
    let hidden = Set(defaults.stringArray(forKey: hiddenFormatsKey) ?? []).union(hiddenByDefault.map(\.id))
    return targets.allSatisfy { hidden.contains($0.id) } ? [] : hidden
}

func visibleFormats(from targets: [Target], rememberingIn defaults: UserDefaults) -> [Target] {
    let hidden = hiddenFormats(in: defaults, for: targets)
    return targets.filter { !hidden.contains($0.id) }
}

func preferredFormat(from targets: [Target], rememberingIn defaults: UserDefaults) -> Target? {
    let visible = visibleFormats(from: targets, rememberingIn: defaults)
    guard !visible.isEmpty else { return nil }
    return visible[selectionIndex(in: visible, rememberingIn: defaults)]
}

private func selectionIndex(in visible: [Target], rememberingIn defaults: UserDefaults) -> Int {
    let recent = defaults.string(forKey: preferenceKey)
    return visible.firstIndex { $0.id == recent } ?? 0
}

@MainActor
func choose(_ targets: [Target], rememberingIn defaults: UserDefaults = preferences,
            copied: CopiedFormat? = nil, suggestedTarget: String? = nil) throws -> Choice {
    guard !targets.isEmpty else { throw Failure("There are no available formats.") }
    let picker = FormatPicker(targets: targets, defaults: defaults, copied: copied, suggestedTarget: suggestedTarget)
    let app = NSApplication.shared
    app.setActivationPolicy(.accessory)
    picker.window.makeKeyAndOrderFront(nil)
    activateForAlert(app, cooperative: false)
    defer { picker.window.orderOut(nil) }
    let response = app.runModal(for: picker.window)
    return Choice(target: response == .OK ? picker.chosenTarget : nil)
}

@MainActor
private final class FormatPickerWindow: NSWindow {
    override func cancelOperation(_ sender: Any?) { NSApp.stopModal(withCode: .cancel) }
}

@MainActor
private final class FormatPicker: NSObject {
    private let columnKey = "copied-md.pickerColumns"
    private let columnOptions = [3, 4, 5]
    private let targets: [Target]
    private let defaults: UserDefaults
    private let copied: CopiedFormat?
    private let suggestedTarget: String?
    private var columns: Int
    private var hidden: Set<String>
    private var selected: Target
    private(set) var chosenTarget: String?
    let window: NSWindow = FormatPickerWindow(contentRect: .zero, styleMask: [.titled, .fullSizeContentView],
                                               backing: .buffered, defer: false)

    init(targets: [Target], defaults: UserDefaults, copied: CopiedFormat?, suggestedTarget: String?) {
        self.targets = targets
        self.defaults = defaults
        self.copied = copied
        self.suggestedTarget = suggestedTarget
        let saved = defaults.integer(forKey: columnKey)
        columns = columnOptions.contains(saved) ? saved : 4
        hidden = hiddenFormats(in: defaults, for: targets)
        selected = targets[0]
        super.init()
        window.title = "Copy Markdown As…"
        window.titleVisibility = .hidden
        window.titlebarAppearsTransparent = true
        window.isMovableByWindowBackground = true
        window.isReleasedWhenClosed = false
        rebuild()
        window.center()
    }

    private func rebuild() {
        let visible = targets.filter { !hidden.contains($0.id) }
        let selectedIndex = visible.firstIndex { $0.id == (copied?.target.id ?? suggestedTarget) }
            ?? selectionIndex(in: visible, rememberingIn: defaults)
        selected = visible[selectedIndex]

        let inset: CGFloat = 24
        let gap: CGFloat = 10
        let cardWidth: CGFloat = 122
        let cardHeight: CGFloat = 90
        let rows = (visible.count + columns - 1) / columns
        let gridWidth = CGFloat(columns) * (cardWidth + gap) - gap
        let fullGridHeight = CGFloat(rows) * (cardHeight + gap) - gap
        let gridHeight = min(fullGridHeight, 4 * (cardHeight + gap) - gap)
        let width = gridWidth + 2 * inset
        let height = gridHeight + (copied?.warning == nil ? 168 : 191)
        let content = NSView(frame: NSRect(x: 0, y: 0, width: width, height: height))

        let title = NSTextField(labelWithString: copied.map { "Copied as \($0.target.label)" } ?? "Copy Markdown")
        title.font = .systemFont(ofSize: 25, weight: .semibold)
        title.frame = NSRect(x: inset, y: height - 56, width: gridWidth - 50, height: 32)
        content.addSubview(title)
        let hint = NSTextField(labelWithString: copied == nil ? "Click a card to copy it or press Return for the selection."
                                    : "Click a different format to replace the copy.")
        hint.font = .systemFont(ofSize: 13)
        hint.textColor = .secondaryLabelColor
        hint.frame = NSRect(x: inset, y: height - 77, width: gridWidth, height: 19)
        content.addSubview(hint)

        if let warning = copied?.warning {
            let note = NSTextField(labelWithString: warning)
            note.font = .systemFont(ofSize: 12)
            note.textColor = .secondaryLabelColor
            note.lineBreakMode = .byTruncatingTail
            note.toolTip = warning
            note.frame = NSRect(x: inset, y: height - 99, width: gridWidth, height: 17)
            content.addSubview(note)
        }

        let settings = settingsButton(visibleCount: visible.count)
        settings.frame = NSRect(x: width - inset - 36, y: height - 57, width: 36, height: 32)
        content.addSubview(settings)

        let grid = NSView(frame: NSRect(x: inset, y: 72, width: gridWidth, height: fullGridHeight))
        for (index, format) in visible.enumerated() {
            let card = FormatCard(format: format, selected: format.id == selected.id, target: self,
                                  action: #selector(copyCard(_:)))
            card.frame = NSRect(x: CGFloat(index % columns) * (cardWidth + gap),
                                y: fullGridHeight - cardHeight - CGFloat(index / columns) * (cardHeight + gap),
                                width: cardWidth, height: cardHeight)
            grid.addSubview(card)
        }
        if fullGridHeight > gridHeight {
            let scroll = NSScrollView(frame: NSRect(x: inset, y: 72, width: gridWidth, height: gridHeight))
            scroll.hasVerticalScroller = true
            scroll.scrollerStyle = .overlay
            scroll.drawsBackground = false
            grid.frame.origin = .zero
            scroll.documentView = grid
            let selectedRow = selectedIndex / columns
            let selectedBottom = fullGridHeight - cardHeight - CGFloat(selectedRow) * (cardHeight + gap)
            scroll.contentView.scroll(to: NSPoint(x: 0, y: min(fullGridHeight - gridHeight, selectedBottom)))
            content.addSubview(scroll)
        } else {
            content.addSubview(grid)
        }

        let line = NSBox(frame: NSRect(x: inset, y: 57, width: gridWidth, height: 1))
        line.boxType = .separator
        content.addSubview(line)

        if copied == nil {
            let cancel = NSButton(title: "Cancel", target: window, action: #selector(NSResponder.cancelOperation(_:)))
            cancel.sizeToFit()
            cancel.frame.origin = NSPoint(x: inset, y: 15)
            content.addSubview(cancel)
        }

        let action = selected.id == copied?.target.id ? "Done" : "Copy \(selected.label)"
        let copy = NSButton(title: action, target: self, action: #selector(copySelected(_:)))
        copy.identifier = NSUserInterfaceItemIdentifier("copy-selected")
        copy.keyEquivalent = "\r"
        copy.sizeToFit()
        copy.frame.size.width = min(max(copy.frame.width + 14, 130), gridWidth - 90)
        copy.frame.origin = NSPoint(x: width - inset - copy.frame.width, y: 15)
        content.addSubview(copy)

        let oldFrame = window.frame
        let frame = window.frameRect(forContentRect: content.frame)
        window.setFrame(NSRect(x: oldFrame.midX - frame.width / 2, y: oldFrame.maxY - frame.height,
                               width: frame.width, height: frame.height), display: false)
        window.contentView = content
        window.defaultButtonCell = copy.cell as? NSButtonCell
        window.initialFirstResponder = copy
        window.makeFirstResponder(copy)
    }

    private func settingsButton(visibleCount: Int) -> NSPopUpButton {
        let button = NSPopUpButton(frame: .zero, pullsDown: true)
        button.identifier = NSUserInterfaceItemIdentifier("picker-settings")
        button.bezelStyle = .rounded
        (button.cell as? NSPopUpButtonCell)?.arrowPosition = .noArrow
        button.toolTip = "Customize format grid"
        button.setAccessibilityLabel("Customize format grid")
        let menu = NSMenu()
        let icon = NSMenuItem(title: "", action: nil, keyEquivalent: "")
        icon.image = NSImage(systemSymbolName: "gearshape", accessibilityDescription: nil)
        menu.addItem(icon)

        let layout = NSMenuItem(title: "Columns", action: nil, keyEquivalent: "")
        let layoutMenu = NSMenu()
        for count in columnOptions {
            let option = NSMenuItem(title: "\(count) Columns", action: #selector(setColumns(_:)), keyEquivalent: "")
            option.tag = count
            option.target = self
            option.state = count == columns ? .on : .off
            layoutMenu.addItem(option)
        }
        layout.submenu = layoutMenu
        menu.addItem(layout)

        let formats = NSMenuItem(title: "Show Formats", action: nil, keyEquivalent: "")
        let formatsMenu = NSMenu()
        formatsMenu.autoenablesItems = false
        for format in targets {
            let option = NSMenuItem(title: format.label, action: #selector(toggleFormat(_:)), keyEquivalent: "")
            option.representedObject = format
            option.target = self
            let isShown = !hidden.contains(format.id)
            option.state = isShown ? .on : .off
            option.isEnabled = !isShown || visibleCount > 1
            formatsMenu.addItem(option)
        }
        formats.submenu = formatsMenu
        menu.addItem(formats)
        button.menu = menu
        button.selectItem(at: 0)
        return button
    }

    @objc private func setColumns(_ item: NSMenuItem) {
        columns = item.tag
        defaults.set(columns, forKey: columnKey)
        rebuild()
    }

    @objc private func toggleFormat(_ item: NSMenuItem) {
        let format = item.representedObject as! Target // Set by settingsButton.
        let id = format.id
        var explicitlyHidden = Set(defaults.stringArray(forKey: hiddenFormatsKey) ?? [])
        var explicitlyShown = Set(defaults.stringArray(forKey: shownFormatsKey) ?? [])
        if item.state == .on {
            explicitlyShown.remove(id)
            if format.enabledByDefault != false { explicitlyHidden.insert(id) }
        } else {
            explicitlyHidden.remove(id)
            if format.enabledByDefault == false { explicitlyShown.insert(id) }
        }
        defaults.set(explicitlyHidden.sorted(), forKey: hiddenFormatsKey)
        defaults.set(explicitlyShown.sorted(), forKey: shownFormatsKey)
        hidden = hiddenFormats(in: defaults, for: targets)
        rebuild()
    }

    @objc private func copyCard(_ card: FormatCard) { finish(with: card.formatID) }
    @objc private func copySelected(_ sender: NSButton) { finish(with: selected.id) }
    private func finish(with id: String) {
        chosenTarget = id
        NSApp.stopModal(withCode: .OK)
    }
}

@MainActor
final class FormatCard: NSButton {
    let formatID: String
    private let mark: FormatIcon
    private let selected: Bool
    private var hovering = false

    init(format: Target, selected: Bool, target: AnyObject, action: Selector) {
        formatID = format.id
        mark = FormatIcon(for: format.id)
        self.selected = selected
        super.init(frame: .zero)
        title = format.label
        identifier = NSUserInterfaceItemIdentifier(format.id)
        self.target = target
        self.action = action
        isBordered = false
        setAccessibilityLabel(format.label)
        setAccessibilitySelected(selected)
        setAccessibilityHelp(selected ? "Selected for Copy. Activate to copy now." : "Activate to copy this format now.")
        toolTip = "Copy \(format.label)"
        addTrackingArea(NSTrackingArea(rect: .zero, options: [.mouseEnteredAndExited, .activeInKeyWindow, .inVisibleRect],
                                       owner: self, userInfo: nil))
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("Use init(format:selected:target:action:)") }

    override func mouseEntered(with event: NSEvent) {
        hovering = true
        needsDisplay = true
    }

    override func mouseExited(with event: NSEvent) {
        hovering = false
        needsDisplay = true
    }

    override func highlight(_ flag: Bool) {
        super.highlight(flag)
        needsDisplay = true
    }

    override var focusRingMaskBounds: NSRect { bounds.insetBy(dx: 2, dy: 2) }

    override func drawFocusRingMask() {
        NSBezierPath(roundedRect: focusRingMaskBounds, xRadius: 13, yRadius: 13).fill()
    }

    override func draw(_ dirtyRect: NSRect) {
        let accent = NSColor.controlAccentColor
        let card = NSBezierPath(roundedRect: focusRingMaskBounds, xRadius: 13, yRadius: 13)
        NSColor.controlBackgroundColor.setFill()
        card.fill()
        let emphasized = hovering || isHighlighted
        let tint = selected ? accent : NSColor.labelColor
        tint.withAlphaComponent(emphasized ? 0.14 : (selected ? 0.09 : 0)).setFill()
        card.fill()
        (selected ? accent : NSColor.separatorColor).setStroke()
        card.lineWidth = selected ? 2 : 1
        card.stroke()

        let badge = NSRect(x: (bounds.width - 36) / 2, y: 16, width: 36, height: 36)
        mark.color.setFill()
        NSBezierPath(roundedRect: badge, xRadius: 10, yRadius: 10).fill()
        mark.image?.draw(in: badge.insetBy(dx: 7, dy: 7))

        if selected {
            let check = NSImage(systemSymbolName: "checkmark.circle.fill", accessibilityDescription: nil)?
                .withSymbolConfiguration(.init(paletteColors: [.white, accent]))
            check?.draw(in: NSRect(x: bounds.width - 24, y: 9, width: 15, height: 15))
        }

        let paragraph = NSMutableParagraphStyle()
        paragraph.alignment = .center
        paragraph.lineBreakMode = .byTruncatingTail
        let label = NSAttributedString(string: formatID == "slack-api" ? "Slack API" : title, attributes: [
            .font: NSFont.systemFont(ofSize: 12, weight: selected ? .semibold : .medium),
            .foregroundColor: NSColor.labelColor,
            .paragraphStyle: paragraph,
        ])
        label.draw(in: NSRect(x: 8, y: 61, width: bounds.width - 16, height: 16))
    }
}
