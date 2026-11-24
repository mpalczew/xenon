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
