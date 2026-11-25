# Chrome Web Store submission

From the repository root, run `pnpm zip` and upload the archive ending in `-chrome.zip`. Use a registered publisher account with 2-Step Verification. [Chrome Web Store Developer Dashboard](https://chrome.google.com/webstore/devconsole)

## Listing

- **Name** copied.md
- **Summary** Copy Markdown in a format that fits the app you’re pasting into.
- **Description** Select Markdown on a page and choose **Copy Markdown As…**, or paste Markdown into the copied.md popup to preview and convert it. Choose rich text, plain text, HTML source, or a format for a supported message composer. You can enable a **Copy as…** button on ChatGPT and Claude replies. Conversion runs on your device.
- **Website** [copied.md on GitHub](https://github.com/0xdoublesharp/copied.md)
- **Privacy policy** [copied.md browser privacy](https://github.com/0xdoublesharp/copied.md/blob/main/docs/browser-extension-privacy.md)

Use the packaged 128 × 128 icon from `apps/browser-extension/src/public/icon-128.png`. Include at least one screenshot at 1280 × 800 or 640 × 400. Use sample Markdown you can publish. [Chrome Web Store listing requirements](https://developer.chrome.com/docs/webstore/cws-dashboard-listing)

## Permission explanations

| Permission                                                | How copied.md uses it                                                                                                          |
| --------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------ |
| `contextMenus`                                            | Shows format choices when the user right-clicks selected text.                                                                 |
| `activeTab`                                               | Lets the extension read selected text in the active tab when the user invokes a copy.                                          |
| `clipboardWrite`                                          | Writes the selected format to the system clipboard.                                                                            |
| `storage`                                                 | Keeps settings and the last-used output format on the device.                                                                  |
| `scripting`                                               | Loads the copy script for a user-invoked copy and registers the script that adds reply buttons on chat sites the user enables. |
| `offscreen`                                               | Provides a document for clipboard writes when the service worker has no page document.                                         |
| `https://chatgpt.com/*`, `https://claude.ai/*` (optional) | Adds a **Copy as…** button to replies on the enabled site. Each site is off until the user grants access in Settings.          |

Check the permission list in the ZIP against these explanations before completing the Privacy tab.

Once the listing is published, install the extension in a clean Chrome profile. Select sample Markdown on a page, choose **Copy Markdown As… → Rich Text**, and paste into an editor. Check that the heading, nested list, and labeled link survive. Repeat from the extension popup.
