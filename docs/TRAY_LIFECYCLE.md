# Asynchronous tray startup (alpha.29)

Scope: the recorded A22 startup wait, with A19/A21/A25 regression coverage.
Before this change, `TrontopApp::new` called `TrayController::new`, which received
a ready message synchronously after `TrayIconBuilder::build`. Failure also joined
the worker on that app-creation path. A slow Shell/driver call could therefore
delay the app's creation. This is a verified code path, not measured evidence
that it caused Trent's earlier closing delay or crash.

## Ownership and wake-up

The controller now returns after starting its one worker, without awaiting tray
construction or joining a failed constructor. The worker creates and destroys
the native tray/menu on its own thread. Its factory result need not be `Send`;
the tests enforce that with an owner-thread-only fake. No retry worker is spawned.

A single latest-sample slot and a private auto-reset event retain the freshest
observation during slow startup/update. There is no queue of stale sample messages.
The sampler never needs a published thread ID. Shutdown signals the same private
event, avoiding message posts to a potentially stale/reused thread ID. The event's
owned handle lives as long as the worker or any sink, including a pending wait.

The worker sleeps on that event plus its own Windows message queue. It drains at
most 64 messages per iteration and rechecks cancellation. This follows Microsoft's
[message-aware wait guidance](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-msgwaitformultipleobjectsex),
including `MWMO_INPUTAVAILABLE` for previously observed but unread messages. The
[auto-reset event](https://learn.microsoft.com/en-us/windows/win32/api/synchapi/nf-synchapi-createeventw)
coalesces wake-ups. No polling timer, global input hook, focus change or new dependency.

## States and shutdown

About shows System tray: Starting in background, Ready, Update failed, Unavailable
or Stopped. Creation failure is terminal for that controller. Update failure stays
visible until a later real sample successfully updates the icon. Only state
transitions and explicit Show/Quit actions request UI repaint, not ordinary tray
samples or hovering. The existing sampler can still request its normal UI repaint.

Cancellation is checked before and after creation. If a slow constructor returns
after close, its late result is destroyed on its owner thread without entering
the update loop. Close during a blocked update stops subsequent updates once the
in-flight call returns. The existing 100 ms shutdown budget remains; it is not a
hard real-time bound under arbitrary OS scheduling. If a native call never returns,
the worker may remain detached until process exit. No unsafe thread termination.

## Verification and limits

Seven new ordinary tests use the production worker/event/message-wait path with a
fake tray backend. They create only private synchronization objects and their
own worker message queues, never a native tray icon or window. Coverage:

- Return while creation is blocked; 10,000 startup samples become one latest update.
- Drop during startup; late backend cleaned on its owner with no update.
- Failed startup reports unavailable, stops accepting samples and never retries.
- Update failure recovers on a real later sample; Show/Quit polling is single-shot.
- 10,000 samples during a blocked update stay bounded; drop does not join forever.
- Successful repeated samples do not request additional UI repaints.
- An already-observed quit message in the worker's own disposable queue wakes
  the production message-aware wait without samples, frames or a native window.

One debug fixture measured controller return at 70.9 microseconds. This is not a
native application-startup benchmark. The About dark/light compact test also
checks the new System tray label/value and existing copy controls remain visible.
No new visual redesign or screenshot claim.

```powershell
cargo test --offline tray -- --nocapture --test-threads=1
cargo test --offline --quiet
cargo clippy --offline --all-targets -- -D warnings
cargo fmt --all --check
cargo build --offline --release
```

Full ordinary suite: 243 passed, 0 failed, 14 ignored. Do not run all ignored tests.
`native_tray_updates_without_any_ui_frames` remains ignored because it creates a
real notification icon. Its native icon/menu/click, hidden-window CPU and teardown
checks require fresh isolated-desktop permission before this rewritten worker can
be called release-verified. Historical A19 evidence does not validate this new pump.
Current user windows/settings were untouched. Exact build identity is in
`CURRENT_STATE.md`; source upload is still unapproved.
