# Worklist

## Next

- [ ] Editor service, like the terminal holders: owns open files and unsaved edits so
      the phone can show, edit and save them, and edits survive a window restart. One
      editor at a time. Needs an `/architect` pass. Cheaper steps if that's too much:
      the phone asks the window over `ipc.sock`, or the window keeps recovery drafts.
- [ ] Phone remote (iPadOS): terminal window occasionally shortens to 8 rows (`8 × 168`)
      while navigating back and forth in Direct mode; pull the `xenon-phone fit` line
      from `journalctl --user -u xenon-phone.service` on the next repro, fix the root
      cause, and remove the temporary `diag` logging.
- [ ] Files sidebar right-click menu shows as a short scrolling stub instead of the
      whole menu (KDE/KWin). Opening it right of the pointer (`files/ops.rs` `popup`)
      didn't help. Next: a full-screen screenshot with the menu open, to see where KWin
      places and sizes it.
