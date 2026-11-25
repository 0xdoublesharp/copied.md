# Verify paste in a destination app

Use the same [Markdown sample](../packages/converters/src/__tests__/fixtures/clipboard-fidelity.md) and composer for both copies. Note the client, its version, and the OS.

1. Open an empty draft in the receiving client. For Teams, open a self-chat and expand the message composer. Use ordinary Paste.
2. Copy the sample as **Rich Text**, paste it, and check the heading, nested list levels, table cells, link text and destination, and indented code.
3. Clear the draft. Enable **Teams** in the format settings, copy the same sample as **Teams**, and paste into the same composer. Check the same structures and clear the draft without sending it.
4. Compare the results. Release a dedicated profile when it preserves more structure without regressing what already works in Rich Text.

For Slack, check that table rows remain readable and the surrounding text still carries emphasis and links. For Signal, check that a rich paste conceals the spoiler and that a plain-text paste includes only the omission marker. For Telegram, record which Telegram client was tested because its rich paste support varies by client.
