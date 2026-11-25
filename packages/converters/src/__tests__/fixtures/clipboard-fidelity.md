## Release readiness

**Ready** for review, with _one_ follow-up and ~~no migration~~.

1. Review the [rollout plan](https://example.org/rollout).
   - Watch the queue.
     - Check pending jobs.
2. Run the smoke test.

| Service | Status                             |
| ------- | ---------------------------------- |
| Web     | Ready                              |
| Worker  | [Logs](https://example.org/worker) |

> Keep the previous build until checks finish.

The inline command is `check --dry-run`.

```python
if ready:
    publish("staging")
```

Reveal this only if needed: <span data-spoiler="true">surprise</span>.
