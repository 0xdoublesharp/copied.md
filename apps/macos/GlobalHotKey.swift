import AppKit
import Carbon.HIToolbox

struct Shortcut: Codable, Equatable {
    let keyCode: UInt32
    let modifiers: UInt32
    let keyLabel: String

    static func defaultShortcut() -> Self {
        Self(keyCode: keyCodeForM() ?? UInt32(kVK_ANSI_M), modifiers: UInt32(controlKey | optionKey | cmdKey), keyLabel: "M")
    }

    private static func keyCodeForM() -> UInt32? {
        guard let input = TISCopyCurrentKeyboardLayoutInputSource()?.takeRetainedValue(),
              let property = TISGetInputSourceProperty(input, kTISPropertyUnicodeKeyLayoutData) else { return nil }
        let layoutData = Unmanaged<CFData>.fromOpaque(property).takeUnretainedValue()
        guard let bytes = CFDataGetBytePtr(layoutData) else { return nil }
        let layout = UnsafeRawPointer(bytes).assumingMemoryBound(to: UCKeyboardLayout.self)
        for key in UInt16(0)..<128 {
            var deadKeyState: UInt32 = 0
            var length = 0
            var characters = [UniChar](repeating: 0, count: 4)
            let status = UCKeyTranslate(
                layout, key, UInt16(kUCKeyActionDisplay), 0, UInt32(LMGetKbdType()),
                OptionBits(kUCKeyTranslateNoDeadKeysMask), &deadKeyState, characters.count, &length, &characters
            )
            if status == noErr && String(decoding: characters.prefix(length), as: UTF16.self).lowercased() == "m" {
                return UInt32(key)
            }
        }
        return nil
    }

    static func record(_ event: NSEvent) -> Self? {
        let flags = event.modifierFlags
        guard flags.contains(.control) || flags.contains(.command) else { return nil }
        var values: [Int] = []
        if flags.contains(.control) { values.append(controlKey) }
        if flags.contains(.option) { values.append(optionKey) }
        if flags.contains(.shift) { values.append(shiftKey) }
        if flags.contains(.command) { values.append(cmdKey) }
        guard values.count >= 2,
              let value = event.charactersIgnoringModifiers?.uppercased(),
              let character = value.unicodeScalars.first,
              value.count == 1, CharacterSet.alphanumerics.contains(character) else { return nil }
        return Self(keyCode: UInt32(event.keyCode), modifiers: values.reduce(0) { $0 | UInt32($1) }, keyLabel: value)
    }

    var label: String {
        [(controlKey, "⌃"), (optionKey, "⌥"), (shiftKey, "⇧"), (cmdKey, "⌘")]
            .filter { modifiers & UInt32($0.0) != 0 }
            .map(\.1).joined() + keyLabel
    }

    func matches(_ other: Self) -> Bool {
        keyCode == other.keyCode && modifiers == other.modifiers
    }
}

@MainActor
final class GlobalHotKey {
    private var eventHandler: EventHandlerRef?
    private var registration: EventHotKeyRef?
    private var identifier: UInt32 = 0
    private(set) var current: Shortcut?
    var onPress: (() -> Void)?

    init() throws {
        var event = EventTypeSpec(eventClass: OSType(kEventClassKeyboard), eventKind: UInt32(kEventHotKeyPressed))
        let status = InstallEventHandler(
            GetApplicationEventTarget(),
            { _, event, owner in
                guard let event, let owner else { return OSStatus(eventNotHandledErr) }
                var key = EventHotKeyID()
                let result = GetEventParameter(
                    event, EventParamName(kEventParamDirectObject), EventParamType(typeEventHotKeyID),
                    nil, MemoryLayout<EventHotKeyID>.size, nil, &key
                )
                guard result == noErr, key.signature == 0x434D4443 else { return OSStatus(eventNotHandledErr) }
                let receiver = Unmanaged<GlobalHotKey>.fromOpaque(owner).takeUnretainedValue()
                let identifier = key.id
                DispatchQueue.main.async { receiver.pressed(identifier) }
                return noErr
            },
            1, &event, Unmanaged.passUnretained(self).toOpaque(), &eventHandler
        )
        guard status == noErr else { throw Failure("macOS could not start the keyboard shortcut (\(status)).") }
    }

    func bind(_ shortcut: Shortcut) throws {
        if let current, current.matches(shortcut) { return }
        let nextID = identifier &+ 1
        var replacement: EventHotKeyRef?
        let status = RegisterEventHotKey(
            shortcut.keyCode, shortcut.modifiers, EventHotKeyID(signature: 0x434D4443, id: nextID),
            GetApplicationEventTarget(), OptionBits(kEventHotKeyExclusive), &replacement
        )
        guard status == noErr, let replacement else {
            if status == eventHotKeyExistsErr { throw Failure("\(shortcut.label) is already in use.") }
            throw Failure("macOS could not register \(shortcut.label) (\(status)).")
        }
        unbind()
        identifier = nextID
        registration = replacement
        current = shortcut
    }

    func unbind() {
        if let registration { UnregisterEventHotKey(registration) }
        registration = nil
        current = nil
    }

    func stop() {
        unbind()
        if let eventHandler { RemoveEventHandler(eventHandler) }
        eventHandler = nil
    }

    private func pressed(_ eventID: UInt32) {
        guard registration != nil, identifier == eventID else { return }
        onPress?()
    }
}
