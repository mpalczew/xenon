# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## State of the repository

New, empty repository (`xero`). No commits, no source code, and no build
tooling exist yet. Update this file with build/test commands and architecture
notes as the project takes shape.

## Rules

`.cursor/rules/core.mdc` holds the user's core AI rules (a copy of their
global `~/.claude/CLAUDE.md`, sourced from personalfiles
`doc/ai_rules/core.md`). Those rules apply here; do not duplicate them in this
file. Key points: minimize cognitive load, present options at real decision
forks before building, Rule of 7, never amend/rebase/force-push, bash not zsh.
