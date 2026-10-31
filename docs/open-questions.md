# Open questions

Decisions we haven't made yet. Resolve one by recording an [ADR](adr/) and removing it from here.

1. **Linux and Windows.** Is cross-platform a real goal, or an option we keep open? This decides how much we invest in abstractions.

2. **Candidate palette acceptance.** Does Regie Dark remain readable in daily
   `htop`, Claude Code and OpenCode work, including selections and tool-owned
   truecolor themes? [Acceptance checklist](testing/regie.md). The identity and
   default/source policy are recorded in [ADR 17](adr/0017-regie-identity-and-theme.md);
   the palette can still be refined from real use.

Resolved: milestone order (window and animation come before the terminal, with a placeholder fill), default hotkey and config format, location and reload behavior ([ADR 5](adr/0005-toml-config-read-at-startup.md)), terminal rendering/emulation ([ADR 8](adr/0008-webview-terminal-and-portable-pty.md)), the menubar and app icon ([ADR 10](adr/0010-menubar-and-app-icon.md)), the MIT licence ([ADR 11](adr/0011-mit-licence.md)), macOS window control: `tao` creates the window and webview, while small `objc2` calls set its level, collection behaviour and activation, so a native `NSPanel` is not needed ([ADR 12](adr/0012-panel-belongs-to-a-display.md)), one running copy ([ADR 13](adr/0013-one-running-copy.md)), and releasing an unsigned DMG ([ADR 14](adr/0014-unsigned-dmg-release.md)).
