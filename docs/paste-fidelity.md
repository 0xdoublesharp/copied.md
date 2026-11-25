# Verify paste in a destination app

Use the same [Markdown sample](../packages/converters/src/__tests__/fixtures/clipboard-fidelity.md) and composer for both copies. Note the client, its version, and the OS.

1. Open an empty draft in the receiving client. For Teams, open a self-chat and expand the message composer. Use ordinary Paste.
2. Copy the sample as **Rich Text**, paste it, and check the heading, nested list levels, visible `☑`/`☐` task checkboxes, table cells, link text and destination, and indented code.
3. Clear the draft. Enable **Teams** or **Word** in the format settings, copy the same sample with that profile, and paste into the same composer in both light and dark modes. Verify code/quote contrast and column alignment, then clear the draft without sending.
4. Compare the results. Release a dedicated profile when it preserves more structure without regressing what already works in Rich Text.

- **Slack:** check that table rows remain aligned in monospace and surrounding text carries emphasis, links, and `☑`/`☐` task markers. For **Slack API (mrkdwn)**, verify pipe characters inside code-block tables do not render literal backslashes.
- **Telegram:** check **Telegram for macOS** (`ru.keepcoder.Telegram`) parses the plain-text Telegram Markdown (`**bold**`, `__italic__`, `~~strike~~`, `` `code` ``, ` ```code``` `, `||spoiler||`) cleanly on send — including bold around inline code without literal backticks — and check **Telegram Desktop** / **Telegram Web** preserve HTML styles and `<tg-spoiler>`.
- **Signal:** check that a rich paste conceals `<span data-spoiler="true">` and preserves bold, italic, strikethrough, and monospace, while plain-text paste includes only `[spoiler omitted]`.
- **Reddit:** check that 4-space-indented code blocks and `>!spoilers!<` render cleanly in both Old Reddit and New Reddit.
- **Reverse HTML → Markdown:** copy formatted content from VS Code, Google Docs, and a web page containing a table, choose **Copy Rich Text as Markdown**, and verify code indentation, inherited style resets, and GFM table structure survive intact.
