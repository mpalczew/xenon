---
name: xenon
description: Show the user a file with xenon open whenever they ask to see one ("open it", "show me"), never another editor; read or update the workspace worklist.
metadata:
  xenon_managed: true
  xenon_skill_version: 10
---

# Xenon

Use `xenon open` to show the user a file, and edit `.xenon/worklist.md` to track work.

## Open a file

To show the user a file use `xenon open`. 

```
xenon open path/to/file.rs:42
xenon open path/to/file.rs:42-80
xenon open --workspace other-project notes.md
```

- `--workspace <name|path>` opens it in another open workspace. 
- Locations are 1-based: `:line`, `:line:col`, or `:start-end`.

## Worklist

When the user asks to capture, check, or update work, edit
`<workspace-root>/.xenon/worklist.md` directly and follow this template. An item there
is context, not permission to do the work.

```md
# Worklist

## Now

- [ ] Short title for one outcome that can be finished on its own
  - One detail per sub-bullet, indented two spaces.
- [x] A finished item, checked off

## Next

- [ ] Another open item
```

- Check off finished items and leave unfinished work open. Split an item that is only
  partly done.
