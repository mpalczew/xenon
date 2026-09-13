# Embedded browser

Xenon’s embedded browser is a WebKit-powered tab, not a second app shell.
The browser chrome stays quiet and local to the tab: back, forward, reload,
address/search field, and an external-browser action. It uses the same pane and
tab model as terminals and editors.

## Opening links

Cmd-click on an `http` or `https` URL presents a compact choice menu at the
pointer:

1. **Open in Xenon** (default)
2. **Open in Default Browser**

The menu is keyboard-first: the first item is selected, Up/Down changes the
selection, Return confirms, and Escape cancels. A normal click keeps the
terminal’s existing behavior. File paths continue to resolve as files.

The browser tab also offers **Open in Default Browser** in its toolbar and
context menu. OAuth and payment flows can therefore be handed off without
copying URLs manually.

## Persistent login state

All normal Xenon browser tabs use one shared `WKWebsiteDataStore` created from:

```text
<xenon data dir>/web-data
```

On a normal install that is `~/.xenon/web-data`. The directory follows the
existing `XENON_DATA_DIR` override, which keeps tests and isolated slots
separate. It is not placed under a workspace because a login should survive:

- closing and reopening Xenon;
- switching workspaces;
- launching a new Xenon version (A → B) with the same data directory.

Private windows use `WKWebsiteDataStore.nonPersistent()` and must never write
to `web-data`.

The browser host owns the shared data-store instance for the process. Tabs own
`WKWebView` instances and share that store. This preserves cookies, local
storage, IndexedDB, and service-worker data while allowing each tab to have
independent navigation state.

## Native host boundary

GPUI owns Xenon’s chrome, but `WKWebView` is an AppKit `NSView`. The macOS
implementation therefore needs a small native host boundary that:

- creates and retains `WKWebView` on the main thread;
- attaches it to the GPUI window’s AppKit content view;
- mirrors the GPUI tab bounds into the native view;
- forwards navigation/title/loading events back into the GPUI tab model;
- removes the view when its tab closes;
- keeps the shared `WKWebsiteDataStore` alive for the application lifetime.

Do not implement this as an image snapshot or a separate browser window. That
would break text selection, login forms, keyboard focus, media, and accessibility.

## Security defaults

- Allow only `http` and `https` navigation by default.
- Open `file:`, custom schemes, and failed navigations externally or show an
  explicit error state.
- Keep JavaScript enabled for normal web compatibility, but expose no native
  bridge until a concrete Xenon use case requires one.
- Handle new-window requests as a choice between a new Xenon tab and the
  default browser.
- Keep private browsing opt-in and visibly labeled.
