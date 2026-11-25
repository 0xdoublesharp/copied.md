# copied.md for VS Code

Select Markdown and choose **Copy Markdown As…**. In a Markdown file, the command also offers to copy the whole document when there is no selection.

Choose **HTML Source** for markup or **Plain Text** for readable text. For rich paste in local VS Code on macOS, install a copied.md app that bundles `Contents/Helpers/copied-md-pasteboard` in **Applications** or your home folder’s **Applications**. VS Code finds the bundled helper automatically. If you keep the helper elsewhere, set its absolute path in `copied-md.pasteboardHelper`. Copies use plain text when rich clipboard access is unavailable. [Bundle the macOS helper](https://github.com/0xdoublesharp/copied.md/blob/main/docs/app-store-and-packaging.md#bundle-the-vs-code-pasteboard-helper)

Use `copied-md.formatVisibility` in VS Code Settings to show or hide formats in **Copy Markdown As…**. For example, `"copied-md.formatVisibility": { "teams": true, "sms": false }` adds Teams and hides SMS in the picker. Word and WhatsApp are also available to enable.
