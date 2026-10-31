# Security policy

## Reporting a vulnerability

Please report security problems privately through GitHub's
[private vulnerability reporting](https://github.com/jdtobe/rake/security/advisories/new).
Do not open a public issue for a vulnerability.

Include what you did, what happened, and which macOS version you are on. You can
expect an acknowledgement within a week. This is a single-maintainer project, so
please allow time for a fix before disclosing publicly.

## Supported versions

Only the latest release is supported. Releases are cut manually, so there may be
a gap between a fix on `main` and a release that contains it.

## Scope

Regie is a local terminal: it runs your login shell through a pseudoterminal and
renders it with a bundled xterm.js in a `wry` webview. It does not listen on the
network, and the terminal page loads no remote content. Problems worth reporting
include anything that lets output from the shell escape the terminal page, read
or write files it should not, or run code without the user's action. Bugs in the
shell itself, in macOS, or in xterm.js should go to those projects.
